use crate::models::TeamName;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use std::fmt;

use crate::models::TeamAbbrev;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GamesResponse {
    pub prev_date: Option<String>,
    pub current_date: Option<String>,
    pub next_date: Option<String>,
    pub games: Vec<GameData>,
}

impl GamesResponse {
    pub fn from_json(data: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(data)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    pub id: u32,
    pub game_type: u8,
    pub venue: Venue,
    #[serde(rename = "startTimeUTC")]
    pub start_time_utc: DateTime<Utc>,
    pub game_state: GameState,
    pub away_team: Team,
    pub home_team: Team,
    pub series_status: Option<SeriesStatus>,
    pub clock: Option<Clock>,
    pub period_descriptor: Option<PeriodDescriptor>, // If the game is not live, there is no PeriodDescriptor
    pub situation: Option<GameSituation>,
    pub goals: Option<Vec<GoalData>>,
    pub game_outcome: Option<GameOutcome>,
}

impl GameData {
    pub fn compute_local_time(&self, tz: Tz) -> DateTime<Tz> {
        self.start_time_utc.with_timezone(&tz)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Venue {
    pub default: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
// Variant names are the API's own uppercase values
#[allow(clippy::upper_case_acronyms)]
pub enum GameState {
    FUT,
    PRE,
    LIVE,
    CRIT,
    OVER,
    FINAL,
    OFF,
    #[serde(other)]
    Unknown,
}

impl GameState {
    /// Whether the game is being played (not counting pregame)
    pub fn is_live(self) -> bool {
        matches!(self, GameState::LIVE | GameState::CRIT)
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeriesStatus {
    pub round: usize,
    pub series_abbrev: String,
    pub series_letter: String,
    pub needed_to_win: usize,
    pub top_seed_team_abbrev: TeamAbbrev,
    pub top_seed_wins: usize,
    pub bottom_seed_team_abbrev: TeamAbbrev,
    pub bottom_seed_wins: usize,
    pub game_number_of_series: usize,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
// Variant names are the API's own uppercase values
#[allow(clippy::upper_case_acronyms)]
pub enum PeriodType {
    REG,
    OT,
    SO,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct Team {
    pub id: u32,
    pub name: TeamName,
    pub abbrev: TeamAbbrev,
    pub score: Option<u8>,
    pub sog: Option<u16>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    #[serde(default)]
    pub time_remaining: String,
    pub running: bool,
    pub in_intermission: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeriodDescriptor {
    pub number: u8,
    pub period_type: PeriodType,
    pub ot_periods: Option<u8>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameSituation {
    pub home_team: TeamSituation,
    pub away_team: TeamSituation,
    pub time_remaining: Option<String>,
    pub situation_code: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TeamSituation {
    pub abbrev: String, // Team name abbrev
    pub strength: u8,
    pub situation_descriptions: Option<Vec<SituationDesc>>,
}

#[derive(Debug, Deserialize)]
pub enum SituationDesc {
    PP,
    EN,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoalData {
    pub period_descriptor: PeriodDescriptor,
    pub time_in_period: String,
    pub player_id: Option<u32>,
    pub first_name: Option<PlayerName>,
    pub last_name: Option<PlayerName>,
    pub goal_modifier: Option<GoalModifier>,
    pub assists: Vec<AssistInfo>,
    pub team_abbrev: TeamAbbrev,
    pub goals_to_date: Option<u16>,
    pub strength: GoalStrength,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PlayerName {
    pub default: String,
}
impl fmt::Display for PlayerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.default)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoalModifier {
    None,
    PenaltyShot,
    EmptyNet,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssistInfo {
    pub player_id: Option<u32>,
    pub name: PlayerName,
    pub assists_to_date: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoalStrength {
    EV,
    SH,
    PP,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameOutcome {
    pub last_period_type: PeriodType,
    pub ot_periods: Option<u8>,
}
