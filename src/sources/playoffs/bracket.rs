use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::models::standings::{SeasonBounds, year_out_of_range_message};
use crate::sources::{BracketResponse, FetchInterval, TabGate, read_json, send_request};
use crate::{AppEvent, Source};

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

/// Poll slowly once every series in the bracket has a winner
fn fetch_interval(finished: bool) -> Duration {
    if finished {
        FetchInterval::InfoLongInterval.as_duration()
    } else {
        FETCH_INTERVAL
    }
}

#[allow(clippy::enum_variant_names)] // Commands read as `BracketCommand::SetX`
pub enum BracketCommand {
    SetYear(i32),
    /// Known seasons, to tell out-of-range years apart from empty brackets
    SetSeasonBounds(Vec<SeasonBounds>),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct BracketSource {
    client: reqwest::Client,
    rx: Receiver<BracketCommand>,
    gate: TabGate,
    current_year: i32,
    seasons: Option<Vec<SeasonBounds>>,
    fetch_interval: Duration,
}
impl BracketSource {
    pub fn new(client: reqwest::Client, rx: Receiver<BracketCommand>, current_year: i32) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_year,
            seasons: None,
            fetch_interval: FETCH_INTERVAL,
        }
    }

    /// Returns whether the fetched bracket is finished
    async fn fetch(&self, tx: &Sender<AppEvent>) -> bool {
        // Skip until the current season has been resolved.
        if self.current_year <= 0 {
            return false;
        }
        // Same out-of-range message as Team Stats for years outside the known
        // seasons, instead of an empty bracket
        if let Some(message) = self
            .seasons
            .as_deref()
            .and_then(|seasons| year_out_of_range_message(self.current_year, seasons))
        {
            log::debug!("Requested bracket year is out of range: {}", message);
            let _ = tx
                .send(AppEvent::BracketUnavailable {
                    year: self.current_year,
                    message,
                })
                .await;
            return false;
        }
        let url = format!(
            "https://api-web.nhle.com/v1/playoff-bracket/{}",
            self.current_year
        );

        let resp = match send_request(self.client.get(&url)).await {
            Ok(resp) => resp,
            Err(err) => {
                log::warn!("Failed to fetch Bracket: {}", err);
                return false;
            }
        };
        let mut finished = false;
        let event = match read_json(resp, "bracket", BracketResponse::from_json).await {
            Some(bracket) => {
                finished = !bracket.series.is_empty()
                    && bracket.series.iter().all(|s| s.winning_team_id.is_some());
                AppEvent::BracketUpdate {
                    year: self.current_year,
                    bracket,
                }
            }
            None => AppEvent::BracketUnavailable {
                year: self.current_year,
                message: "No playoff bracket available for this season.".to_string(),
            },
        };
        let _ = tx.send(event).await;
        finished
    }

    fn set_fetch_interval(&mut self, finished: bool, interval: &mut tokio::time::Interval) {
        let period = fetch_interval(finished);
        if period != self.fetch_interval {
            log::debug!("Setting bracket interval to {:?}", period);
            self.fetch_interval = period;
            *interval = tokio::time::interval_at(Instant::now() + period, period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        }
    }
}

#[async_trait::async_trait]
impl Source for BracketSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(self.fetch_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        BracketCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, self.fetch_interval) {
                                interval.reset_at(due);
                            }
                        }
                        BracketCommand::SetSeasonBounds(seasons) => {
                            self.seasons = Some(seasons);
                            if self.gate.target_changed() {
                                let finished = self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                                self.set_fetch_interval(finished, &mut interval);
                            }
                        }
                        BracketCommand::SetYear(year) => {
                            self.current_year = year;
                            if self.gate.target_changed() {
                                let finished = self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                                self.set_fetch_interval(finished, &mut interval);
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
