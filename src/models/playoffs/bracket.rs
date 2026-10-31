use serde::Deserialize;

use crate::models::{TeamAbbrev, TeamName};

#[derive(Debug, Deserialize, Default)]
pub struct BracketResponse {
    /// The playoff-bracket endpoint returns HTTP 200 with `{}` for seasons with
    /// no playoffs
    #[serde(default)]
    pub series: Vec<Series>,
}

impl BracketResponse {
    pub fn from_json(data: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(data)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub series_title: String,
    pub series_abbrev: String,
    pub series_letter: String,
    pub playoff_round: u8,
    pub top_seed_rank: u8,
    pub top_seed_rank_abbrev: String,
    pub top_seed_wins: u8,
    pub bottom_seed_rank: u8,
    pub bottom_seed_rank_abbrev: String,
    pub bottom_seed_wins: u8,
    pub top_seed_team: Option<SeriesTeam>,
    pub bottom_seed_team: Option<SeriesTeam>,
    pub winning_team_id: Option<i32>,
    pub losing_team_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesTeam {
    pub id: i32,
    pub abbrev: TeamAbbrev,
    pub name: TeamName,
    pub common_name: TeamName,
    pub wins: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::BracketResponse;

    #[test]
    fn empty_object_parses_as_empty_bracket() {
        // The API returns HTTP 200 `{}` for no-playoff/invalid years. This must
        // parse (not error on the missing `series` field) so the Playoffs tab
        // can show an out-of-range hint rather than failing to parse.
        let parsed = BracketResponse::from_json("{}").expect("{} should parse");
        assert!(parsed.series.is_empty());
    }

    #[test]
    fn populated_bracket_parses() {
        let json = r#"{"series":[{"seriesTitle":"Stanley Cup Final","seriesAbbrev":"SCF","seriesLetter":"O","playoffRound":4,"topSeedRank":1,"topSeedRankAbbrev":"A1","topSeedWins":4,"bottomSeedRank":2,"bottomSeedRankAbbrev":"M2","bottomSeedWins":1,"topSeedTeam":null,"bottomSeedTeam":null,"winningTeamId":null,"losingTeamId":null}]}"#;
        let parsed = BracketResponse::from_json(json).expect("should parse");
        assert_eq!(parsed.series.len(), 1);
        assert_eq!(parsed.series[0].series_letter, "O");
    }
}
