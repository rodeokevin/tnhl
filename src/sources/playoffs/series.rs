use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::sources::{FetchInterval, SeriesResponse, TabGate, read_json, send_request};
use crate::{AppEvent, Source};

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

/// Poll slowly once a team has won the series
fn fetch_interval(finished: bool) -> Duration {
    if finished {
        FetchInterval::InfoLongInterval.as_duration()
    } else {
        FETCH_INTERVAL
    }
}

#[allow(clippy::enum_variant_names)] // Commands read as `SeriesCommand::SetX`
pub enum SeriesCommand {
    SetYear(i32),
    SetSeries(Option<char>),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct SeriesSource {
    client: reqwest::Client,
    rx: Receiver<SeriesCommand>,
    gate: TabGate,
    current_year: i32,
    series_letter: Option<char>,
    fetch_interval: Duration,
}
impl SeriesSource {
    pub fn new(
        client: reqwest::Client,
        rx: Receiver<SeriesCommand>,
        current_year: i32,
        series_letter: Option<char>,
    ) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_year,
            series_letter,
            fetch_interval: FETCH_INTERVAL,
        }
    }

    /// Returns whether the fetched series is finished
    async fn fetch(&self, tx: &Sender<AppEvent>) -> bool {
        if let Some(letter) = &self.series_letter {
            let url = format!(
                "https://api-web.nhle.com/v1/schedule/playoff-series/{}{}/{}",
                self.current_year - 1,
                self.current_year,
                letter,
            );
            let resp = match send_request(self.client.get(&url)).await {
                Ok(resp) => resp,
                Err(err) => {
                    log::warn!("Failed to fetch series: {}", err);
                    return false;
                }
            };
            let mut finished = false;
            let event = match read_json(resp, "series", SeriesResponse::from_json).await {
                Some(parsed_series) => {
                    let needed = parsed_series.needed_to_win;
                    finished = needed > 0
                        && (parsed_series.top_seed_team.series_wins >= needed
                            || parsed_series.bottom_seed_team.series_wins >= needed);
                    AppEvent::SeriesUpdate {
                        year: self.current_year,
                        parsed_series,
                    }
                }
                None => AppEvent::SeriesUnavailable {
                    year: self.current_year,
                    letter: letter.to_string(),
                    message: "No series data available.".to_string(),
                },
            };
            let _ = tx.send(event).await;
            finished
        } else {
            log::debug!("Not fetching series data because no series is selected");
            false
        }
    }

    fn set_fetch_interval(&mut self, finished: bool, interval: &mut tokio::time::Interval) {
        let period = fetch_interval(finished);
        if period != self.fetch_interval {
            log::debug!("Setting series interval to {:?}", period);
            self.fetch_interval = period;
            *interval = tokio::time::interval_at(Instant::now() + period, period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        }
    }
}

#[async_trait::async_trait]
impl Source for SeriesSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(self.fetch_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        SeriesCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, self.fetch_interval) {
                                interval.reset_at(due);
                            }
                        }
                        SeriesCommand::SetYear(year) => {
                            self.current_year = year;
                            // No series should be selected when the year changes
                            self.series_letter = None;
                        }
                        SeriesCommand::SetSeries(letter) => {
                            if let Some(letter) = letter {
                                log::debug!("Fetching new series data for {} series: {}", self.current_year, letter);
                                self.series_letter = Some(letter);
                                if self.gate.target_changed() {
                                    let finished = self.fetch(&tx).await;
                                    self.gate.fetched();
                                    interval.reset();
                                    self.set_fetch_interval(finished, &mut interval);
                                }
                            } else {
                                // Series closed: stop polling it
                                self.series_letter = None;
                            }
                        }
                    }
                },
                _ = interval.tick(), if self.gate.is_active() => {
                    let finished = self.fetch(&tx).await;
                    self.gate.fetched();
                    self.set_fetch_interval(finished, &mut interval);
                }
            }
        }
    }
}
