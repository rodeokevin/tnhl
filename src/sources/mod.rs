use std::time::Duration;
use tokio::sync::mpsc::Sender;

use crate::models::{
    TeamAbbrev,
    games::play_by_play::PlaysResponse,
    games::{boxscore::BoxscoreResponse, game_story::GameStoryResponse, games::GamesResponse},
    playoffs::{bracket::BracketResponse, series::SeriesResponse},
    standings::{SeasonBounds, StandingsResponse},
    team_stats::TeamStatsResponse,
};

pub mod games;
pub mod playoffs;
pub mod season;
pub mod standings;
pub mod teams_stats;
pub mod today;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum FetchInterval {
    GamesShortInterval,
    GamesLongInterval,
    InfoShortInterval,
    InfoLongInterval,
    PlaysShortInterval,
}
impl FetchInterval {
    pub fn as_duration(&self) -> Duration {
        match self {
            FetchInterval::GamesShortInterval => Duration::from_secs(10),
            FetchInterval::GamesLongInterval => Duration::from_secs(60),
            FetchInterval::InfoShortInterval => Duration::from_secs(30),
            FetchInterval::InfoLongInterval => Duration::from_secs(600),
            FetchInterval::PlaysShortInterval => Duration::from_secs(15),
        }
    }
}

/// Max requests in flight at once across all sources
const MAX_CONCURRENT_REQUESTS: usize = 2;
static REQUEST_PERMITS: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(MAX_CONCURRENT_REQUESTS);

/// Send a request, waiting for a free slot first, and log responses that
/// aren't 200 OK. The response is returned unchanged.
pub async fn send_request(request: reqwest::RequestBuilder) -> reqwest::Result<reqwest::Response> {
    // The semaphore is never closed, so acquiring can't fail
    let _permit = REQUEST_PERMITS
        .acquire()
        .await
        .expect("request semaphore closed");
    request.send().await.inspect(log_status)
}

/// Read and parse a response body. An error status, an unreadable body or
/// JSON that doesn't parse all count as "no data" (`None`), logged with
/// `what`, so callers can show a message instead of loading forever.
pub async fn read_json<T>(
    resp: reqwest::Response,
    what: &str,
    parse: impl FnOnce(&str) -> Result<T, serde_json::Error>,
) -> Option<T> {
    // Non-200 statuses are already logged by `send_request`
    if !resp.status().is_success() {
        return None;
    }
    match resp.text().await {
        Ok(body) => parse(&body)
            .inspect_err(|e| log::error!("Failed to parse {}: {}", what, e))
            .ok(),
        Err(e) => {
            log::warn!("Failed to read {}: {}", what, e);
            None
        }
    }
}

/// Log a response that isn't 200
fn log_status(resp: &reqwest::Response) {
    let status = resp.status();
    if status != reqwest::StatusCode::OK {
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok());
        match retry_after {
            Some(retry_after) => log::error!(
                "{} returned {} (retry-after: {})",
                resp.url(),
                status,
                retry_after
            ),
            None => log::error!("{} returned {}", resp.url(), status),
        }
    }
}

/// Game and date changes wait this long before fetching
pub const FETCH_DEBOUNCE: Duration = Duration::from_millis(250);

pub const RECENT_FETCH_WINDOW: Duration = Duration::from_secs(15);

/// Pauses a source while its tab is hidden. Changes to what the source
/// fetches (date, team, game, ...) are deferred until the tab is shown.
#[derive(Default)]
pub struct TabGate {
    active: bool,
    /// The target changed while hidden, so the held data is out of date
    stale: bool,
    last_fetch: Option<tokio::time::Instant>,
}

impl TabGate {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Call when the source's target changes. Returns whether to fetch now;
    /// while hidden, the fetch waits until the tab is shown.
    pub fn target_changed(&mut self) -> bool {
        if !self.active {
            self.stale = true;
        }
        self.active
    }

    pub fn fetched(&mut self) {
        self.stale = false;
        self.last_fetch = Some(tokio::time::Instant::now());
    }

    /// Show or hide the tab. When newly shown, returns when the next fetch is
    /// due: immediately if the data went stale or is older than `period`.
    pub fn set_active(&mut self, active: bool, period: Duration) -> Option<tokio::time::Instant> {
        let shown = active && !self.active;
        self.active = active;
        if !shown {
            return None;
        }
        let now = tokio::time::Instant::now();
        Some(match self.last_fetch {
            Some(at) if !self.stale => (at + period).max(now),
            _ => now,
        })
    }
}

/// Events sent to the main application loop.
#[derive(Debug)]
pub enum AppEvent {
    /// The current NHL season (end year) resolved at startup from season bounds.
    SeasonResolved {
        year: i32,
    },
    /// Today's NHL game day changed while the app was running
    TodayChanged {
        date: chrono::NaiveDate,
    },
    SeasonBoundsResolved {
        seasons: Vec<SeasonBounds>,
    },
    StandingsUpdate {
        /// The date that was requested, YYYY-MM-DD
        date: String,
        standings: StandingsResponse,
        season: Option<SeasonBounds>,
    },
    StandingsOutOfRange {
        /// The date that was requested, YYYY-MM-DD
        date: String,
        message: String,
    },
    /// The standings request for `date` failed or returned unusable data
    StandingsUnavailable {
        date: String,
        message: String,
    },
    /// A team stats request failed or returned unusable data
    TeamStatsUnavailable {
        team: TeamAbbrev,
        year: i32,
        /// Which request failed: playoffs or regular season
        playoffs: bool,
        message: String,
    },
    TeamStatsRegularSeasonUpdate {
        team: TeamAbbrev,
        year: i32,
        stats: TeamStatsResponse,
    },
    TeamStatsPlayoffsUpdate {
        team: TeamAbbrev,
        year: i32,
        stats: TeamStatsResponse,
    },
    TeamStatsOutOfRange {
        team: TeamAbbrev,
        year: i32,
        message: String,
    },
    GamesUpdate {
        parsed_games: GamesResponse,
    },
    /// The scoreboard for the current date couldn't be loaded
    GamesUnavailable {
        /// The requested date, YYYY-MM-DD
        date: String,
        message: String,
    },
    /// A per-game request failed or returned unusable data
    BoxscoreUnavailable {
        game_id: u32,
    },
    PlaysUnavailable {
        game_id: u32,
    },
    GameStoryUnavailable {
        game_id: u32,
    },
    BoxscoreUpdate {
        game_id: u32,
        parsed_boxscore: BoxscoreResponse,
    },
    PlaysUpdate {
        game_id: u32,
        parsed_plays: PlaysResponse,
    },
    GameStoryUpdate {
        game_id: u32,
        parsed_game_story: GameStoryResponse,
    },
    /// The bracket request for `year` failed or returned unusable data
    BracketUnavailable {
        year: i32,
        message: String,
    },
    /// The series request failed or returned unusable data
    SeriesUnavailable {
        year: i32,
        letter: String,
        message: String,
    },
    BracketUpdate {
        /// Season end year the bracket was fetched for
        year: i32,
        bracket: BracketResponse,
    },
    SeriesUpdate {
        /// Season end year the series was fetched for
        year: i32,
        parsed_series: SeriesResponse,
    },
    TotalGoalsUpdate {
        game_id: u32,
        parsed_series: SeriesResponse,
    },
    BracketSeriesUpdate {
        /// Season id the series was fetched for, e.g. "19291930"
        season: String,
        letter: String,
        parsed_series: SeriesResponse,
    },
    Input(crossterm::event::KeyEvent),
    /// Periodic tick to refresh UI
    Tick,
    /// A termination signal arrived: quit like `q`
    Shutdown,
}

// async_trait marks its boxed future #[must_use], which clippy flags as redundant
#[allow(clippy::double_must_use)]
#[async_trait::async_trait]
pub trait Source: Send + 'static {
    async fn run(
        self: Box<Self>,
        tx: Sender<AppEvent>,
        cancel: tokio_util::sync::CancellationToken,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: Duration = Duration::from_secs(60);

    #[test]
    fn first_show_fetches_immediately() {
        let mut gate = TabGate::default();
        let due = gate.set_active(true, PERIOD).unwrap();
        assert!(due <= tokio::time::Instant::now());
    }

    #[test]
    fn target_change_while_hidden_waits_for_show() {
        let mut gate = TabGate::default();
        gate.set_active(true, PERIOD);
        gate.fetched();
        gate.set_active(false, PERIOD);
        assert!(!gate.target_changed());
        // Stale data is fetched as soon as the tab is shown again
        let due = gate.set_active(true, PERIOD).unwrap();
        assert!(due <= tokio::time::Instant::now());
    }

    #[test]
    fn fresh_data_resumes_schedule_on_show() {
        let mut gate = TabGate::default();
        gate.set_active(true, PERIOD);
        gate.fetched();
        gate.set_active(false, PERIOD);
        let due = gate.set_active(true, PERIOD).unwrap();
        assert!(due > tokio::time::Instant::now() + PERIOD - Duration::from_secs(1));
    }

    #[test]
    fn target_change_while_shown_fetches_now() {
        let mut gate = TabGate::default();
        gate.set_active(true, PERIOD);
        assert!(gate.target_changed());
        // Already shown: no new schedule
        assert!(gate.set_active(true, PERIOD).is_none());
    }
}
