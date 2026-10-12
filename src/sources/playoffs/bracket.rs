use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::models::standings::{SeasonBounds, year_out_of_range_message};
use crate::sources::{BracketResponse, TabGate, read_json, send_request};
use crate::{AppEvent, Source};

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

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
}
impl BracketSource {
    pub fn new(client: reqwest::Client, rx: Receiver<BracketCommand>, current_year: i32) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_year,
            seasons: None,
        }
    }

    async fn fetch(&self, tx: &Sender<AppEvent>) {
        // Skip until the current season has been resolved.
        if self.current_year <= 0 {
            return;
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
            return;
        }
        let url = format!(
            "https://api-web.nhle.com/v1/playoff-bracket/{}",
            self.current_year
        );

        let resp = match send_request(self.client.get(&url)).await {
            Ok(resp) => resp,
            Err(err) => {
                log::warn!("Failed to fetch Bracket: {}", err);
                return;
            }
        };
        let event = match read_json(resp, "bracket", BracketResponse::from_json).await {
            Some(bracket) => AppEvent::BracketUpdate {
                year: self.current_year,
                bracket,
            },
            None => AppEvent::BracketUnavailable {
                year: self.current_year,
                message: "No playoff bracket available for this season.".to_string(),
            },
        };
        let _ = tx.send(event).await;
    }
}

#[async_trait::async_trait]
impl Source for BracketSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(FETCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        BracketCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, FETCH_INTERVAL) {
                                interval.reset_at(due);
                            }
                        }
                        BracketCommand::SetSeasonBounds(seasons) => {
                            self.seasons = Some(seasons);
                            if self.gate.target_changed() {
                                self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                            }
                        }
                        BracketCommand::SetYear(year) => {
                            self.current_year = year;
                            if self.gate.target_changed() {
                                self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
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
