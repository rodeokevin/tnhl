use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::models::playoffs::series::SeriesResponse;
use crate::sources::{TabGate, send_request};
use crate::{AppEvent, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketSeriesTarget {
    pub season: String,
    pub letter: String,
}

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

pub enum BracketSeriesCommand {
    SetTargets(Vec<BracketSeriesTarget>),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct BracketSeriesSource {
    client: reqwest::Client,
    rx: Receiver<BracketSeriesCommand>,
    gate: TabGate,
    targets: Vec<BracketSeriesTarget>,
}

impl BracketSeriesSource {
    pub fn new(client: reqwest::Client, rx: Receiver<BracketSeriesCommand>) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            targets: Vec::new(),
        }
    }

    async fn fetch(&self, tx: &Sender<AppEvent>) {
        let fetches = self.targets.iter().map(|target| {
            let client = &self.client;
            async move {
                let letter = target.letter.to_lowercase();
                let url = format!(
                    "https://api-web.nhle.com/v1/schedule/playoff-series/{}/{}",
                    target.season, letter
                );

                match send_request(client.get(&url)).await {
                    Ok(resp) => {
                        if let Ok(body) = resp.text().await {
                            match SeriesResponse::from_json(&body) {
                                Ok(parsed_series) => {
                                    let _ = tx
                                        .send(AppEvent::BracketSeriesUpdate {
                                            season: target.season.clone(),
                                            letter: target.letter.clone(),
                                            parsed_series,
                                        })
                                        .await;
                                }
                                Err(e) => log::error!(
                                    "Failed to parse bracket series {}: {}",
                                    target.letter,
                                    e
                                ),
                            }
                        }
                    }
                    Err(err) => {
                        log::warn!("Failed to fetch bracket series {}: {}", target.letter, err)
                    }
                }
            }
        });

        futures::future::join_all(fetches).await;
    }
}

#[async_trait::async_trait]
impl Source for BracketSeriesSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(FETCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,

                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        BracketSeriesCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, FETCH_INTERVAL) {
                                interval.reset_at(due);
                            }
                        }
                        BracketSeriesCommand::SetTargets(mut targets) => {
                            targets.sort_by(|a, b| {
                                (a.season.as_str(), a.letter.as_str())
                                    .cmp(&(b.season.as_str(), b.letter.as_str()))
                            });
                            let mut current = self.targets.clone();
                            current.sort_by(|a, b| {
                                (a.season.as_str(), a.letter.as_str())
                                    .cmp(&(b.season.as_str(), b.letter.as_str()))
                            });
                            if targets != current {
                                log::debug!("Fetching bracket series because targets changed");
                                self.targets = targets;
                                if self.gate.target_changed() {
                                    self.fetch(&tx).await;
                                    self.gate.fetched();
                                    interval.reset();
                                }
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
