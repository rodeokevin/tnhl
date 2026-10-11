use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::sources::{FetchInterval, GamesResponse, TabGate, send_request};
use crate::{AppEvent, Source};

#[allow(clippy::enum_variant_names)] // Commands read as `GamesCommand::SetX`
pub enum GamesCommand {
    SetDate(String),
    SetInterval(Duration),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct GamesSource {
    client: reqwest::Client,
    rx: Receiver<GamesCommand>,
    gate: TabGate,
    current_date: String,
    fetch_interval: Duration,
}
impl GamesSource {
    pub fn new(client: reqwest::Client, rx: Receiver<GamesCommand>, current_date: String) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_date,
            fetch_interval: FetchInterval::GamesShortInterval.as_duration(),
        }
    }

    async fn fetch(&self, tx: &Sender<AppEvent>) {
        let url = format!("https://api-web.nhle.com/v1/score/{}", self.current_date);

        let resp = match send_request(self.client.get(&url)).await {
            Ok(resp) => resp,
            Err(err) => {
                log::warn!("Failed to fetch games: {}", err);
                return;
            }
        };

        // An error status (e.g. the 500 the NHL API returns for parts of the
        // 1987 Canada Cup) or an unreadable body would otherwise leave the tab
        // loading forever
        let parsed = if resp.status().is_success() {
            match resp.text().await {
                Ok(body) => GamesResponse::from_json(&body)
                    .inspect_err(|e| log::error!("Failed to parse games: {}", e))
                    .ok(),
                Err(e) => {
                    log::warn!("Failed to read games body: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let event = match parsed {
            Some(parsed_games) => AppEvent::GamesUpdate { parsed_games },
            None => AppEvent::GamesUnavailable {
                date: self.current_date.clone(),
                message: "No game data available for this date.".to_string(),
            },
        };
        let _ = tx.send(event).await;
    }
}

#[async_trait::async_trait]
impl Source for GamesSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(self.fetch_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,

                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        GamesCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, self.fetch_interval) {
                                interval.reset_at(due);
                            }
                        }
                        GamesCommand::SetDate(date) => {
                            self.current_date = date;
                            if self.gate.target_changed() {
                                self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                            }
                        },
                        GamesCommand::SetInterval(new_interval) => {
                            if new_interval != self.fetch_interval {
                                log::debug!("Setting games interval to {:?}", new_interval);
                                self.fetch_interval = new_interval;

                                interval = tokio::time::interval_at(Instant::now() + self.fetch_interval, self.fetch_interval);
                                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
