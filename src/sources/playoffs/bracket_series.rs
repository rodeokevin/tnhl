use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::models::playoffs::series::SeriesResponse;
use crate::sources::{FetchInterval, TabGate, send_request};
use crate::{AppEvent, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketSeriesTarget {
    pub season: String,
    pub letter: String,
}

const FETCH_INTERVAL: Duration = Duration::from_secs(60);

/// Poll slowly once every series has loaded; targets are only past seasons
fn fetch_interval(finished: bool) -> Duration {
    if finished {
        FetchInterval::InfoLongInterval.as_duration()
    } else {
        FETCH_INTERVAL
    }
}

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
    fetch_interval: Duration,
}

impl BracketSeriesSource {
    pub fn new(client: reqwest::Client, rx: Receiver<BracketSeriesCommand>) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            targets: Vec::new(),
            fetch_interval: FETCH_INTERVAL,
        }
    }

    /// Returns whether every series loaded
    async fn fetch(&self, tx: &Sender<AppEvent>) -> bool {
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
                                    return true;
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
                false
            }
        });

        let loaded = futures::future::join_all(fetches).await;
        !loaded.is_empty() && loaded.into_iter().all(|ok| ok)
    }

    fn set_fetch_interval(&mut self, finished: bool, interval: &mut tokio::time::Interval) {
        let period = fetch_interval(finished);
        if period != self.fetch_interval {
            log::debug!("Setting bracket series interval to {:?}", period);
            self.fetch_interval = period;
            *interval = tokio::time::interval_at(Instant::now() + period, period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        }
    }
}

#[async_trait::async_trait]
impl Source for BracketSeriesSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(self.fetch_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,

                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        BracketSeriesCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, self.fetch_interval) {
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
                                    let finished = self.fetch(&tx).await;
                                    self.gate.fetched();
                                    interval.reset();
                                    self.set_fetch_interval(finished, &mut interval);
                                }
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
