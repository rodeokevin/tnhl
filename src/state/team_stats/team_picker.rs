use crate::models::TeamAbbrev;
use std::str::FromStr;

#[derive(Debug)]
pub enum InputError {
    InvalidLength,
    InvalidTeam,
}

pub struct TeamPickerState {
    pub is_valid: bool,
    /// Current user input in the date picker
    pub text: String,
    /// Current team
    pub current_team: TeamAbbrev,
}

impl TeamPickerState {
    /// Validate the input date
    pub fn validate_input(&mut self) -> Result<TeamAbbrev, InputError> {
        let input: String = self.text.drain(..).collect();
        let input = input.trim();

        if input.len() != 3 || !input.chars().all(|c| c.is_ascii_alphabetic()) {
            self.is_valid = false;
            return Err(InputError::InvalidLength);
        }

        match TeamAbbrev::from_str(input) {
            // Event teams (All-Stars, national teams) have no team stats
            Ok(team) if team.is_franchise() => Ok(team),
            _ => {
                self.is_valid = false;
                Err(InputError::InvalidTeam)
            }
        }
    }
    /// Set the team from the validated input string from the team picker
    pub fn set_team_from_valid_input(&mut self, team: TeamAbbrev) {
        self.current_team = team;
    }
}

impl Default for TeamPickerState {
    fn default() -> Self {
        Self {
            is_valid: true,
            text: String::new(),
            current_team: TeamAbbrev::default(),
        }
    }
}
