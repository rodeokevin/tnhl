use serde::Deserialize;
use std::fmt;

use crate::models::{
    TeamAbbrev, TeamAbbrevWrapper,
    games::{
        Position,
        games::{AssistInfo, GoalModifier, GoalStrength, PeriodDescriptor, PlayerName},
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStoryResponse {
    pub away_team: Option<StoryTeam>,
    pub home_team: Option<StoryTeam>,
    pub summary: Option<Summary>,
    pub pre_game_matchup: Option<PreGameMatchup>,
}

impl GameStoryResponse {
    pub fn from_json(data: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(data)
    }
}

/// Top-level team info in a game story, including the season record.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryTeam {
    pub abbrev: Option<TeamAbbrev>,
    pub record: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreGameMatchup {
    pub skating_leaders: SkatingLeaders,
    pub goalie_comparison: GoalieComparison,
    pub team_season_stats: TeamSeasonStats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkatingLeaders {
    /// How many recent games the leaders cover (e.g. 5 for the last 5 games).
    pub game_limit: Option<u16>,
    pub leaders: Vec<LeaderCategory>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderCategory {
    pub category: String,
    pub away_leader: Option<Leader>,
    pub home_leader: Option<Leader>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leader {
    pub name: PlayerName,
    pub sweater_number: Option<u16>,
    pub position_code: Position,
    pub value: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalieComparison {
    pub away_team: Vec<GoalieCompare>,
    pub home_team: Vec<GoalieCompare>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalieCompare {
    pub name: PlayerName,
    pub games_played: Option<u16>,
    pub record: Option<String>,
    pub gaa: Option<f64>,
    pub save_pctg: Option<f64>,
    pub shutouts: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSeasonStats {
    pub away_team: TeamSeasonStat,
    pub home_team: TeamSeasonStat,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSeasonStat {
    pub pp_pctg: f64,
    pub pk_pctg: f64,
    pub faceoff_winning_pctg: f64,
    pub goals_for_per_game_played: f64,
    pub goals_against_per_game_played: f64,
    pub pp_pctg_rank: u16,
    pub pk_pctg_rank: u16,
    pub faceoff_winning_pctg_rank: u16,
    pub goals_for_per_game_played_rank: u16,
    pub goals_against_per_game_played_rank: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub scoring: Vec<PeriodScore>,
    pub shootout: Vec<ShootoutAttempt>,
    pub team_game_stats: Vec<TeamGameStats>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodScore {
    pub period_descriptor: PeriodDescriptor,
    pub goals: Vec<StoryGoalData>,
}

// Essentially the same as games::GoalData but the team_abbrev field has an
// extra `default` field in it
#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StoryGoalData {
    pub time_in_period: String,
    pub player_id: u32,
    pub first_name: PlayerName,
    pub last_name: PlayerName,
    pub goal_modifier: GoalModifier,
    pub assists: Vec<AssistInfo>,
    pub team_abbrev: TeamAbbrevWrapper,
    pub goals_to_date: Option<u16>,
    pub strength: GoalStrength,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShootoutAttempt {
    pub sequence: usize,
    pub player_id: u32,
    pub team_abbrev: TeamAbbrevWrapper,
    pub first_name: PlayerName,
    pub last_name: PlayerName,
    pub result: ShootoutAttemptResult,
    pub home_score: usize,
    pub away_score: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShootoutAttemptResult {
    Save,
    Goal,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamGameStats {
    pub category: GameStatsCategory,
    pub away_value: StatValue,
    pub home_value: StatValue,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum GameStatsCategory {
    Sog,
    FaceoffWinningPctg,
    PowerPlay,
    PowerPlayPctg,
    Pim,
    Hits,
    BlockedShots,
    Giveaways,
    Takeaways,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StatValue {
    Int(u16),
    Float(f64),
    Str(String),
}

impl StatValue {
    pub fn is_zero(&self) -> bool {
        match self {
            StatValue::Int(v) => *v == 0,
            StatValue::Float(v) => *v == 0.0,
            StatValue::Str(_) => false,
        }
    }
}

impl fmt::Display for StatValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatValue::Int(v) => write!(f, "{}", v),
            StatValue::Float(v) => write!(f, "{}", (v * 100.0).round() as u8),
            StatValue::Str(v) => write!(f, "{}", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A trimmed but structurally-representative pre-game game-story payload.
    const PREGAME_JSON: &str = r#"{
        "awayTeam": {"abbrev": "CHI", "record": "29-39-14"},
        "homeTeam": {"abbrev": "DET", "record": "40-30-12"},
        "preGameMatchup": {
            "skatingLeaders": {
                "leaders": [
                    {
                        "category": "points",
                        "awayLeader": {"name": {"default": "C. Bedard"}, "sweaterNumber": 98, "positionCode": "C", "value": 75},
                        "homeLeader": {"name": {"default": "A. DeBrincat"}, "sweaterNumber": 93, "positionCode": "R", "value": 85}
                    },
                    {
                        "category": "plusMinus",
                        "awayLeader": {"name": {"default": "T. Bertuzzi"}, "sweaterNumber": 59, "positionCode": "L", "value": -12},
                        "homeLeader": null
                    }
                ]
            },
            "goalieComparison": {
                "awayTeam": [
                    {"name": {"default": "S. Knight"}, "gamesPlayed": 55, "record": "19-25-11", "gaa": 2.82, "savePctg": 0.902, "shutouts": 3}
                ],
                "homeTeam": []
            },
            "teamSeasonStats": {
                "awayTeam": {"ppPctg": 0.169492, "pkPctg": 0.836134, "faceoffWinningPctg": 0.46013, "goalsForPerGamePlayed": 2.56, "goalsAgainstPerGamePlayed": 3.29, "ppPctgRank": 29, "pkPctgRank": 2, "faceoffWinningPctgRank": 31, "goalsForPerGamePlayedRank": 30, "goalsAgainstPerGamePlayedRank": 27},
                "homeTeam": {"ppPctg": 0.225806, "pkPctg": 0.771429, "faceoffWinningPctg": 0.510032, "goalsForPerGamePlayed": 2.91, "goalsAgainstPerGamePlayed": 3.1, "ppPctgRank": 12, "pkPctgRank": 23, "faceoffWinningPctgRank": 10, "goalsForPerGamePlayedRank": 22, "goalsAgainstPerGamePlayedRank": 19}
            }
        }
    }"#;

    #[test]
    fn parses_pre_game_matchup() {
        let resp = GameStoryResponse::from_json(PREGAME_JSON).expect("should parse");
        assert_eq!(
            resp.away_team.as_ref().and_then(|t| t.record.clone()),
            Some("29-39-14".to_string())
        );
        assert_eq!(
            resp.away_team.as_ref().and_then(|t| t.abbrev),
            Some(TeamAbbrev::CHI)
        );
        let matchup = resp.pre_game_matchup.expect("has matchup");

        assert_eq!(matchup.skating_leaders.leaders.len(), 2);
        let points = &matchup.skating_leaders.leaders[0];
        assert_eq!(points.category, "points");
        assert_eq!(points.away_leader.as_ref().unwrap().value, 75);
        // plusMinus can be negative.
        let plus_minus = &matchup.skating_leaders.leaders[1];
        assert_eq!(plus_minus.away_leader.as_ref().unwrap().value, -12);
        assert!(plus_minus.home_leader.is_none());

        assert_eq!(matchup.goalie_comparison.away_team.len(), 1);
        assert!(matchup.goalie_comparison.home_team.is_empty());
        assert_eq!(
            matchup.goalie_comparison.away_team[0].record,
            Some("19-25-11".to_string())
        );

        assert_eq!(matchup.team_season_stats.away_team.pp_pctg_rank, 29);
    }

    #[test]
    fn missing_pre_game_matchup_is_none() {
        let resp = GameStoryResponse::from_json(r#"{"summary": null}"#).expect("should parse");
        assert!(resp.pre_game_matchup.is_none());
    }
}
