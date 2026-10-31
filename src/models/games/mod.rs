use serde::Deserialize;
use std::fmt;

pub mod boxscore;
pub mod game_story;
pub mod games;
pub mod play_by_play;
pub mod score_now;

#[derive(Debug, Clone, Deserialize)]
pub enum Position {
    #[serde(rename = "L")]
    LeftWing,
    #[serde(rename = "R")]
    RightWing,
    #[serde(rename = "C")]
    Center,
    #[serde(rename = "D")]
    Defense,
    #[serde(rename = "G")]
    Goalie,
    /// Some old international games list skaters as plain forwards
    #[serde(rename = "F")]
    Forward,
    /// Any other code, so one odd player doesn't fail the whole response
    #[serde(other)]
    Unknown,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Position::LeftWing => "LW",
            Position::RightWing => "RW",
            Position::Center => "C",
            Position::Defense => "D",
            Position::Goalie => "G",
            Position::Forward => "F",
            Position::Unknown => "-",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn forward_and_unknown_positions_parse() {
        let parsed: Vec<Position> = serde_json::from_str(r#"["C","F","X"]"#).unwrap();
        assert!(matches!(
            parsed.as_slice(),
            [Position::Center, Position::Forward, Position::Unknown]
        ));
        assert_eq!(Position::Forward.to_string(), "F");
    }
}
