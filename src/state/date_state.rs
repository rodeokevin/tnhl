use chrono::{Datelike, NaiveDate, ParseError, Utc};
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
    pub fn validate_input_date(&mut self, tz: Tz) -> Result<NaiveDate, ParseError> {
        let input: String = std::mem::take(&mut self.text);
        let date = match input.as_str() {
            "t" | "today" => Ok(Utc::now().with_timezone(&tz).date_naive()),
            _ => NaiveDate::parse_from_str(input.as_str(), "%Y-%m-%d"),
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
    pub fn validate_input_year(&mut self, tz: Tz) -> Result<i32, ()> {
        let input: String = std::mem::take(&mut self.text);

        let year = match input.as_str() {
            // Use the bounds-resolved current season if available
            "t" | "today" => Ok(self
                .current_season_year
                .unwrap_or_else(|| Utc::now().with_timezone(&tz).year())),
            _ => input.parse::<i32>().map_err(|_| ()),
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
    pub fn set_date_with_arrows(&mut self, forward: bool) -> NaiveDate {
        match forward {
            true => self.date_selection_offset += 1,
            false => self.date_selection_offset -= 1,
        }
        self.date + chrono::Duration::days(self.date_selection_offset)
    }
    /// Set the year using Left/Right arrow keys
    pub fn set_year_with_arrows(&mut self, forward: bool) -> i32 {
        match forward {
            true => self.year_selection_offset += 1,
            false => self.year_selection_offset -= 1,
        }
        self.year + self.year_selection_offset
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
