use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::sources::{SeriesResponse, TabGate, read_json, send_request};
use crate::{AppEvent, Source};

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

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
        }
    }

    async fn fetch(&self, tx: &Sender<AppEvent>) {
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
                    return;
                }
            };
            let event = match read_json(resp, "series", SeriesResponse::from_json).await {
                Some(parsed_series) => AppEvent::SeriesUpdate {
                    year: self.current_year,
                    parsed_series,
                },
                None => AppEvent::SeriesUnavailable {
                    year: self.current_year,
                    letter: letter.to_string(),
                    message: "No series data available.".to_string(),
                },
            };
            let _ = tx.send(event).await;
        } else {
            log::debug!("Not fetching series data because no series is selected");
        }
    }
}

#[async_trait::async_trait]
impl Source for SeriesSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(FETCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        SeriesCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, FETCH_INTERVAL) {
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
                                    self.fetch(&tx).await;
                                    self.gate.fetched();
                                    interval.reset();
                                }
                            } else {
                                // Series closed: stop polling it
                                self.series_letter = None;
                            }
                        }
                    }
                },
                _ = interval.tick(), if self.gate.is_active() => {
                    self.fetch(&tx).await;
                    self.gate.fetched();
                }
            }
        }
    }
}
