use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::sources::{BracketResponse, TabGate, send_request};
use crate::{AppEvent, Source};

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

pub enum BracketCommand {
    SetYear(i32),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct BracketSource {
    client: reqwest::Client,
    rx: Receiver<BracketCommand>,
    gate: TabGate,
    current_year: i32,
}
impl BracketSource {
    pub fn new(client: reqwest::Client, rx: Receiver<BracketCommand>, current_year: i32) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_year,
        }
    }

    async fn fetch(&self, tx: &Sender<AppEvent>) {
        // Skip until the current season has been resolved.
        if self.current_year <= 0 {
            return;
        }
        let url = format!(
            "https://api-web.nhle.com/v1/playoff-bracket/{}",
            self.current_year
        );

        match send_request(self.client.get(&url)).await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    // Parse the JSON
                    match BracketResponse::from_json(&body) {
                        Ok(parsed_playoff_bracket) => {
                            log::debug!("Bracket data successfully parsed");
                            let _ = tx
                                .send(AppEvent::BracketUpdate {
                                    year: self.current_year,
                                    bracket: parsed_playoff_bracket,
                                })
                                .await;
                            log::debug!("Sent Bracket data to app");
                        }
                        Err(e) => log::error!("Failed to parse Bracket: {}", e),
                    }
                }
            }
            Err(err) => log::warn!("Failed to fetch Bracket: {}", err),
        }
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
