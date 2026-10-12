use chrono::{Datelike, NaiveDate, Utc};

use crate::models::standings::season_end_year_fallback;
use chrono_tz::Tz;

/// Get user input for the date and store whether it's valid.
pub struct DateState {
    pub is_valid: bool,
    /// Current user input in the date picker
    pub text: String,
    /// Current date for games and standings (this is configured by App.configure() on startup)
    pub date: NaiveDate,
    /// Today's game day (from the NHL API when available)
    pub today: NaiveDate,
    pub year: i32,
    /// The bounds-resolved current-season end year (set from SeasonResolved).
    /// Used by the year picker's "t"/"today" shortcut so it picks the correct
    /// season during the offseason / early season rather than the raw calendar
    /// year.
    pub current_season_year: Option<i32>,
    /// Used for selecting the date or year with arrow keys.
    pub date_selection_offset: i64,
    pub year_selection_offset: i32,
}

impl DateState {
    /// Validate the input date
    /// Years before 1 (0 and negative) are rejected
    pub fn validate_input_date(&mut self) -> Result<NaiveDate, ()> {
        let input: String = std::mem::take(&mut self.text);
        let date = match input.as_str() {
            // The NHL game day, which stays on the previous date until late
            // games finish, rather than the wall-clock date
            "t" | "today" => Ok(self.today),
            _ => NaiveDate::parse_from_str(input.as_str(), "%Y-%m-%d")
                .ok()
                .filter(|date| date.year() >= 1)
                .ok_or(()),
        };
        self.is_valid = date.is_ok();
        date
    }
    /// Set the date from the validated input string from the date picker
    pub fn set_date_from_valid_input(&mut self, date: NaiveDate) {
        self.date = date;
        self.date_selection_offset = 0;
    }
    /// Validate the input year
    /// Years before 1 (0 and negative) are rejected; 0 also means "unset"
    pub fn validate_input_year(&mut self, tz: Tz) -> Result<i32, ()> {
        let input: String = std::mem::take(&mut self.text);

        let year = match input.as_str() {
            // Use the bounds-resolved current season if available
            "t" | "today" => Ok(self.current_season_year.unwrap_or_else(|| {
                season_end_year_fallback(Utc::now().with_timezone(&tz).date_naive())
            })),
            _ => input
                .parse::<i32>()
                .ok()
                .filter(|&year| year >= 1)
                .ok_or(()),
        };

        self.is_valid = year.is_ok();
        year
    }
    /// Set the year from the validated input string from the date picker
    pub fn set_year_from_valid_input(&mut self, year: i32) {
        self.year = year;
        self.year_selection_offset = 0;
    }

    /// Set the date using Left/Right arrow keys to move a single day at a time
    /// Stops at the earliest/latest representable date instead of overflowing
    pub fn set_date_with_arrows(&mut self, forward: bool) -> NaiveDate {
        let offset = self.date_selection_offset + if forward { 1 } else { -1 };
        match self.date.checked_add_signed(chrono::Duration::days(offset)) {
            Some(date) => {
                self.date_selection_offset = offset;
                date
            }
            None => self
                .date
                .checked_add_signed(chrono::Duration::days(self.date_selection_offset))
                .unwrap_or(self.date),
        }
    }
    /// Set the year using Left/Right arrow keys
    /// Stops at the smallest/largest year instead of overflowing
    pub fn set_year_with_arrows(&mut self, forward: bool) -> i32 {
        let offset = self.year_selection_offset + if forward { 1 } else { -1 };
        match self.year.checked_add(offset) {
            Some(year) => {
                self.year_selection_offset = offset;
                year
            }
            None => self.year.saturating_add(self.year_selection_offset),
        }
    }
    /// Format the data to be used in the title of a border;
    pub fn format_date_border_title(&self) -> String {
        self.date.format(" %B %d, %Y ").to_string()
    }
    pub fn move_date_selector_by_arrow(&mut self, right_arrow: bool) {
        let date = self.set_date_with_arrows(right_arrow);
        self.text.clear();
        self.text.push_str(&date.to_string());
    }
    pub fn move_year_selector_by_arrow(&mut self, right_arrow: bool) {
        let year = self.set_year_with_arrows(right_arrow);
        self.text.clear();
        self.text.push_str(&year.to_string());
    }
}

impl Default for DateState {
    fn default() -> Self {
        DateState {
            is_valid: true,
            text: String::new(),
            date: Utc::now().date_naive(),
            today: Utc::now().date_naive(),
            year: 0,
            current_season_year: None,
            date_selection_offset: 0,
            year_selection_offset: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_picker_rejects_year_zero_and_negative_years() {
        let mut state = DateState::default();
        for input in ["-0001-01-01", "0000-01-01", "-2026-10-10"] {
            state.text = input.to_string();
            assert_eq!(state.validate_input_date(), Err(()), "{input}");
            assert!(!state.is_valid);
        }
        state.text = "0001-01-01".to_string();
        assert!(state.validate_input_date().is_ok());
    }

    #[test]
    fn year_picker_rejects_year_zero_and_negative_years() {
        let mut state = DateState::default();
        for input in ["-5", "0", "-2026"] {
            state.text = input.to_string();
            assert_eq!(
                state.validate_input_year(chrono_tz::UTC),
                Err(()),
                "{input}"
            );
            assert!(!state.is_valid);
        }
        state.text = "1".to_string();
        assert_eq!(state.validate_input_year(chrono_tz::UTC), Ok(1));
    }

    #[test]
    fn today_shortcut_uses_the_nhl_game_day() {
        // Just after midnight the NHL game day is still the previous date
        let game_day = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let mut state = DateState {
            today: game_day,
            text: "t".to_string(),
            ..DateState::default()
        };
        assert_eq!(state.validate_input_date(), Ok(game_day));
    }

    #[test]
    fn arrows_stop_at_the_latest_date_instead_of_panicking() {
        let mut state = DateState {
            date: NaiveDate::MAX,
            ..DateState::default()
        };
        assert_eq!(state.set_date_with_arrows(true), NaiveDate::MAX);
        assert_eq!(
            state.set_date_with_arrows(false),
            NaiveDate::MAX.pred_opt().unwrap()
        );
    }

    #[test]
    fn arrows_stop_at_the_largest_year_instead_of_overflowing() {
        let mut state = DateState {
            year: i32::MAX,
            ..DateState::default()
        };
        assert_eq!(state.set_year_with_arrows(true), i32::MAX);
        assert_eq!(state.set_year_with_arrows(false), i32::MAX - 1);
    }
}
