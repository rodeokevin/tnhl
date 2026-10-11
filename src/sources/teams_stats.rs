use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{AppEvent, Source, TabGate, send_request};
use crate::models::TeamAbbrev;
use crate::models::standings::{SeasonBounds, YearResolution, resolve_year};
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

        if let Some(seasons) = self.seasons.as_deref() {
            match resolve_year(self.current_year, seasons) {
                Some(YearResolution::AfterLatest(latest)) => {
                    let msg = format!(
                        "No stats for this season yet. Latest available: {}-{}.",
                        latest - 1,
                        latest
                    );
                    log::debug!("Requested team-stats year is after latest season: {}", msg);
                    let _ = tx
                        .send(AppEvent::TeamStatsOutOfRange { message: msg })
                        .await;
                    return;
                }
                Some(YearResolution::BeforeEarliest(earliest)) => {
                    let msg = format!(
                        "No stats for this season. Earliest available: {}-{}.",
                        earliest - 1,
                        earliest
                    );
                    log::debug!(
                        "Requested team-stats year is before earliest season: {}",
                        msg
                    );
                    let _ = tx
                        .send(AppEvent::TeamStatsOutOfRange { message: msg })
                        .await;
                    return;
                }
                _ => {}
            }
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
                        .send(AppEvent::TeamStatsOutOfRange { message: msg })
                        .await;
                    return;
                }
                if let Ok(body) = resp.text().await {
                    // Parse the JSON
                    match TeamStatsResponse::from_json(&body) {
                        Ok(mut parsed_team_stats) => {
                            log::debug!("Regular season team stats data successfully parsed");
                            // Sort by points for skaters
                            parsed_team_stats
                                .skaters
                                .sort_by_key(|s| std::cmp::Reverse(s.points));
                            // Sort by games played for goalies
                            parsed_team_stats
                                .goalies
                                .sort_by_key(|s| std::cmp::Reverse(s.games_played));
                            let _ = tx
                                .send(AppEvent::TeamStatsRegularSeasonUpdate(parsed_team_stats))
                                .await;
                            log::debug!("Sent regular season team stats data to app");
                        }
                        Err(e) => log::error!("Failed to parse regular season team stats: {}", e),
                    }
                }
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
                if let Ok(body) = resp.text().await {
                    // Parse the JSON
                    match TeamStatsResponse::from_json(&body) {
                        Ok(mut parsed_team_stats) => {
                            log::debug!("Playoffs team stats data successfully parsed");
                            // Sort by points for skaters
                            parsed_team_stats
                                .skaters
                                .sort_by_key(|s| std::cmp::Reverse(s.points));
                            // Sort by games played for goalies
                            parsed_team_stats
                                .goalies
                                .sort_by_key(|s| std::cmp::Reverse(s.games_played));
                            let _ = tx
                                .send(AppEvent::TeamStatsPlayoffsUpdate(parsed_team_stats))
                                .await;
                            log::debug!("Sent playoffs team stats data to app");
                        }
                        Err(e) => log::error!("Failed to parse playoffs team stats: {}", e),
                    }
                }
            }
            Err(err) => log::warn!("Failed to fetch playoffs team stats: {}", err),
        }
    }
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
