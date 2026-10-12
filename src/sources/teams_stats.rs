use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{AppEvent, Source, TabGate, read_json, send_request};
use crate::models::TeamAbbrev;
use crate::models::standings::{SeasonBounds, year_out_of_range_message};
use crate::models::team_stats::TeamStatsResponse;

/// Team stats change at most once per game, so a fixed interval is plenty
const FETCH_INTERVAL: Duration = Duration::from_secs(60);

#[allow(clippy::enum_variant_names)] // Commands read as `TeamStatsCommand::SetX`
pub enum TeamStatsCommand {
    SetTeam(TeamAbbrev),
    SetYear(i32),
    SetSeasonBounds(Vec<SeasonBounds>),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct TeamStatsSource {
    client: reqwest::Client,
    rx: Receiver<TeamStatsCommand>,
    gate: TabGate,
    current_team: TeamAbbrev,
    current_year: i32,
    seasons: Option<Vec<SeasonBounds>>,
}
impl TeamStatsSource {
    pub fn new(
        client: reqwest::Client,
        rx: Receiver<TeamStatsCommand>,
        current_team: TeamAbbrev,
        current_year: i32,
    ) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_team,
            current_year,
            seasons: None,
        }
    }

    async fn fetch(&self, tx: &Sender<AppEvent>) {
        // Skip until the current season has been resolved
        if self.current_year <= 0 {
            return;
        }

        if let Some(message) = self
            .seasons
            .as_deref()
            .and_then(|seasons| year_out_of_range_message(self.current_year, seasons))
        {
            log::debug!("Requested team-stats year is out of range: {}", message);
            let _ = tx
                .send(AppEvent::TeamStatsOutOfRange {
                    team: self.current_team,
                    year: self.current_year,
                    message,
                })
                .await;
            return;
        }

        let regular_season_url = format!(
            "https://api-web.nhle.com/v1/club-stats/{}/{}{}/2",
            self.current_team,
            self.current_year - 1,
            self.current_year,
        );

        match send_request(self.client.get(&regular_season_url)).await {
            Ok(resp) => {
                // A 404 on the regular-season endpoint means the team didn't
                // play that season
                if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    let msg = format!(
                        "{} did not play in the {}-{} season.",
                        self.current_team,
                        self.current_year - 1,
                        self.current_year,
                    );
                    log::debug!("Team stats 404: {}", msg);
                    let _ = tx
                        .send(AppEvent::TeamStatsOutOfRange {
                            team: self.current_team,
                            year: self.current_year,
                            message: msg,
                        })
                        .await;
                    return;
                }
                let event = match read_json(resp, "regular season team stats", parse_stats).await {
                    Some(stats) => AppEvent::TeamStatsRegularSeasonUpdate {
                        team: self.current_team,
                        year: self.current_year,
                        stats,
                    },
                    None => self.unavailable(false),
                };
                let _ = tx.send(event).await;
            }
            Err(err) => log::warn!("Failed to fetch regular season team stats: {}", err),
        }

        let playoffs_url = format!(
            "https://api-web.nhle.com/v1/club-stats/{}/{}{}/3",
            self.current_team,
            self.current_year - 1,
            self.current_year,
        );

        match send_request(self.client.get(&playoffs_url)).await {
            Ok(resp) => {
                let event = match read_json(resp, "playoffs team stats", parse_stats).await {
                    Some(stats) => AppEvent::TeamStatsPlayoffsUpdate {
                        team: self.current_team,
                        year: self.current_year,
                        stats,
                    },
                    None => self.unavailable(true),
                };
                let _ = tx.send(event).await;
            }
            Err(err) => log::warn!("Failed to fetch playoffs team stats: {}", err),
        }
    }
}

impl TeamStatsSource {
    /// A request for the current team/season failed or returned unusable data
    fn unavailable(&self, playoffs: bool) -> AppEvent {
        AppEvent::TeamStatsUnavailable {
            team: self.current_team,
            year: self.current_year,
            playoffs,
            message: "No team stats available for this season.".to_string(),
        }
    }
}

/// Parse team stats, sorting skaters by points and goalies by games played
fn parse_stats(body: &str) -> Result<TeamStatsResponse, serde_json::Error> {
    let mut stats = TeamStatsResponse::from_json(body)?;
    stats.skaters.sort_by_key(|s| std::cmp::Reverse(s.points));
    stats
        .goalies
        .sort_by_key(|s| std::cmp::Reverse(s.games_played));
    Ok(stats)
}

#[async_trait::async_trait]
impl Source for TeamStatsSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(FETCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        TeamStatsCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, FETCH_INTERVAL) {
                                interval.reset_at(due);
                            }
                        }
                        TeamStatsCommand::SetTeam(team) => {
                            self.current_team = team;
                            if self.gate.target_changed() {
                                self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                            }
                        }
                        TeamStatsCommand::SetYear(year) => {
                            self.current_year = year;
                            if self.gate.target_changed() {
                                self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                            }
                        }
                        // Bounds are resolved exactly once by SeasonSource
                        TeamStatsCommand::SetSeasonBounds(seasons) => {
                            self.seasons = Some(seasons);
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
