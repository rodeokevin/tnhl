use std::collections::HashMap;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::models::games::{game_story::GameStoryResponse, games::GameState};
use crate::sources::{
    FETCH_DEBOUNCE, FetchInterval, RECENT_FETCH_WINDOW, TabGate, read_json, send_request,
};
use crate::{AppEvent, Source};

pub enum GameStoryCommand {
    /// The game shown on the Games tab, with its latest state
    SetGame(Option<(u32, GameState)>),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct GameStorySource {
    client: reqwest::Client,
    rx: Receiver<GameStoryCommand>,
    gate: TabGate,
    game: Option<(u32, GameState)>,
    /// When each game's story was last fetched successfully
    last_fetched: HashMap<u32, Instant>,
    fetch_interval: Duration,
}

/// Poll fast only while the selected game is being played
fn fetch_interval(state: Option<GameState>) -> Duration {
    match state {
        Some(state) if state.is_live() => FetchInterval::InfoShortInterval.as_duration(),
        _ => FetchInterval::InfoLongInterval.as_duration(),
    }
}

impl GameStorySource {
    pub fn new(client: reqwest::Client, rx: Receiver<GameStoryCommand>) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            game: None,
            last_fetched: HashMap::new(),
            fetch_interval: fetch_interval(None),
        }
    }

    fn fetched_recently(&self, game_id: u32) -> bool {
        self.last_fetched
            .get(&game_id)
            .is_some_and(|at| at.elapsed() < RECENT_FETCH_WINDOW)
    }

    async fn fetch(&mut self, tx: &Sender<AppEvent>) {
        let Some((game_id, _)) = self.game else {
            return;
        };
        let url = format!("https://api-web.nhle.com/v1/wsc/game-story/{}", game_id);

        let resp = match send_request(self.client.get(&url)).await {
            Ok(resp) => resp,
            Err(err) => {
                log::warn!(
                    "Failed to fetch game story for game id {}: {}",
                    game_id,
                    err
                );
                return;
            }
        };
        // Any response counts as a fetch, failures included, so a game that just
        // failed (e.g. rate limited) isn't refetched within the recent window
        self.last_fetched.insert(game_id, Instant::now());
        let event = match read_json(resp, "game story", GameStoryResponse::from_json).await {
            Some(parsed) => AppEvent::GameStoryUpdate {
                game_id,
                parsed_game_story: parsed,
            },
            None => AppEvent::GameStoryUnavailable { game_id },
        };
        let _ = tx.send(event).await;
    }
}

#[async_trait::async_trait]
impl Source for GameStorySource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(self.fetch_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Deadline for a debounced fetch after the target changed
        let mut pending: Option<Instant> = None;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,

                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        GameStoryCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, self.fetch_interval) {
                                interval.reset_at(due);
                            }
                        }
                        GameStoryCommand::SetGame(game) => {
                            let previous = std::mem::replace(&mut self.game, game);
                            let period = fetch_interval(game.map(|(_, state)| state));
                            if period != self.fetch_interval {
                                log::debug!("Setting game story interval to {:?}", period);
                                self.fetch_interval = period;
                                // Wait a full period: a game change fetches via the debounce below
                                interval = tokio::time::interval_at(Instant::now() + period, period);
                                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                            }
                            // Sent on every GamesUpdate, so most of these are no-ops
                            match (previous, game) {
                                // No selection means the date changed and the app
                                // dropped its game story, so nothing held is recent anymore
                                (_, None) => {
                                    self.last_fetched.clear();
                                    pending = None;
                                }
                                // Same game: fetch once when its state changes (e.g. it
                                // went live or ended) so the slow interval doesn't hide it
                                (Some((prev_id, prev_state)), Some((id, state))) if prev_id == id => {
                                    if prev_state != state && self.gate.target_changed() {
                                        log::debug!("Fetching game story because game {} changed state", id);
                                        self.fetch(&tx).await;
                                        self.gate.fetched();
                                        interval.reset();
                                    }
                                }
                                (_, Some((id, _))) if self.fetched_recently(id) => {
                                    log::debug!("Not refetching game story for game {}: fetched recently", id);
                                    pending = None;
                                }
                                (_, Some(_)) => {
                                    log::debug!("Fetching game story because selected game changed");
                                    if self.gate.target_changed() {
                                        pending = Some(Instant::now() + FETCH_DEBOUNCE);
                                    }
                                }
                            }
                        },
                    }
                },
                _ = tokio::time::sleep_until(pending.unwrap_or_else(Instant::now)), if pending.is_some() => {
                    pending = None;
                    self.fetch(&tx).await;
                    self.gate.fetched();
                    interval.reset();
                }
                _ = interval.tick(), if self.gate.is_active() => {
                    self.fetch(&tx).await;
                    self.gate.fetched();
                }
            }
        }
    }
}
