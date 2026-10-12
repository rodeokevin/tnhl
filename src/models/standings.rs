use serde::Deserialize;

use crate::models::{TeamAbbrevWrapper, TeamName};

#[derive(Debug, Deserialize)]
pub struct StandingsResponse {
    pub standings: Vec<TeamData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamData {
    pub team_name: TeamName,
    pub team_abbrev: TeamAbbrevWrapper,
    pub season_id: u32,
    pub clinch_indicator: Option<String>,
    pub conference_abbrev: Option<String>,
    pub conference_name: Option<String>,
    pub division_abbrev: Option<String>,
    pub division_name: Option<String>,
    pub conference_sequence: u8,
    pub wildcard_sequence: u8,
    pub division_sequence: u8,
    pub league_sequence: u8,
    pub games_played: u16,
    pub wins: u8,
    pub losses: u8,
    pub ot_losses: u8,
    pub points: u16,
    pub point_pctg: Option<f64>,
    pub regulation_wins: u8,
    pub regulation_plus_ot_wins: u8,
    pub goal_for: u16,
    pub goal_against: u16,
    pub home_wins: u8,
    pub home_ot_losses: u8,
    pub home_losses: u8,
    pub road_wins: u8,
    pub road_ot_losses: u8,
    pub road_losses: u8,
    pub shootout_wins: u8,
    pub shootout_losses: u8,
    pub l10_wins: u8,
    pub l10_ot_losses: u8,
    pub l10_losses: u8,
    pub streak_code: Option<String>,
    pub streak_count: Option<u8>,
}

impl StandingsResponse {
    pub fn from_json(data: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(data)
    }

    pub fn divisions(&self) -> Vec<Grouping> {
        self.groupings(
            |t| t.division_abbrev.as_deref(),
            |t| t.division_name.as_deref(),
        )
    }

    pub fn conferences(&self) -> Vec<Grouping> {
        self.groupings(
            |t| t.conference_abbrev.as_deref(),
            |t| t.conference_name.as_deref(),
        )
    }

    pub fn has_wildcard(&self) -> bool {
        self.standings.iter().any(|t| t.wildcard_sequence != 0)
    }

    pub fn divisions_in_conference(&self, conference_abbrev: &str) -> Vec<Grouping> {
        let mut seen: Vec<Grouping> = Vec::new();
        for team in &self.standings {
            if team.conference_abbrev.as_deref() != Some(conference_abbrev) {
                continue;
            }
            let Some(code) = team.division_abbrev.as_deref() else {
                continue;
            };
            if seen.iter().any(|g| g.abbrev == code) {
                continue;
            }
            seen.push(Grouping {
                abbrev: code.to_string(),
                name: team.division_name.as_deref().unwrap_or(code).to_string(),
            });
        }
        sort_by_display_order(&mut seen);
        seen
    }

    fn groupings<'a>(
        &'a self,
        abbrev: impl Fn(&'a TeamData) -> Option<&'a str>,
        name: impl Fn(&'a TeamData) -> Option<&'a str>,
    ) -> Vec<Grouping> {
        let mut seen: Vec<Grouping> = Vec::new();
        for team in &self.standings {
            let Some(code) = abbrev(team) else { continue };
            if seen.iter().any(|g| g.abbrev == code) {
                continue;
            }
            seen.push(Grouping {
                abbrev: code.to_string(),
                // Fall back to the abbrev if the name is missing.
                name: name(team).unwrap_or(code).to_string(),
            });
        }
        sort_by_display_order(&mut seen);
        seen
    }
}

/// Preferred display order
const DISPLAY_ORDER: [&str; 6] = ["E", "W", "A", "M", "C", "P"];

fn sort_by_display_order(groupings: &mut [Grouping]) {
    groupings.sort_by_key(|g| {
        DISPLAY_ORDER
            .iter()
            .position(|a| *a == g.abbrev)
            .unwrap_or(DISPLAY_ORDER.len())
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grouping {
    pub abbrev: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct StandingsSeasonResponse {
    pub seasons: Vec<SeasonBounds>,
}

/// The date range a season's standings are valid for.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonBounds {
    pub id: u32,
    pub standings_start: String,
    pub standings_end: String,
}

impl StandingsSeasonResponse {
    pub fn from_json(data: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(data)
    }
}

impl SeasonBounds {
    /// Parsed start date, if valid.
    pub fn start(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(&self.standings_start, "%Y-%m-%d").ok()
    }
    /// Parsed end date, if valid.
    pub fn end(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(&self.standings_end, "%Y-%m-%d").ok()
    }
    pub fn end_year(&self) -> i32 {
        (self.id % 10000) as i32
    }
}

/// Best guess at the current season's end year without the season bounds.
/// Seasons end the following year, and preseason games can start in
/// September, so September to December belong to next year's season.
pub fn season_end_year_fallback(date: chrono::NaiveDate) -> i32 {
    use chrono::Datelike;
    if date.month() >= 9 {
        date.year() + 1
    } else {
        date.year()
    }
}

/// Resolve the NHL "season end year" for a given date from the season bounds.
///
/// This is the year used to build season identifiers (e.g. `20262027`) for
/// team stats and playoffs:
/// - if `date` is within a season, that season's end year;
/// - if `date` is in an offseason gap or after the latest season, the most
///   recent completed season's end year
/// - if `date` is before the earliest season, the earliest season's end year.
pub fn season_end_year_for_date(date: chrono::NaiveDate, seasons: &[SeasonBounds]) -> Option<i32> {
    // Season containing the date.
    if let Some(s) = seasons.iter().find(|s| match (s.start(), s.end()) {
        (Some(start), Some(end)) => date >= start && date <= end,
        _ => false,
    }) {
        return Some(s.end_year());
    }

    // Otherwise the most recent season that ended before the date.
    if let Some(s) = seasons
        .iter()
        .filter(|s| s.end().is_some_and(|end| end < date))
        .max_by_key(|s| s.end())
    {
        return Some(s.end_year());
    }

    // Otherwise (date before any season) the earliest season's end year.
    seasons
        .iter()
        .filter(|s| s.end().is_some())
        .min_by_key(|s| s.end())
        .map(|s| s.end_year())
}

/// Classification of a requested standings date relative to the known seasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateResolution {
    /// Date falls within a season
    InSeason(chrono::NaiveDate),
    /// Date is in an offseason gap between two seasons so show the most recent
    /// completed season by fetching its end date
    OffseasonGap(chrono::NaiveDate),
    /// Date is after the latest known season end
    AfterLatest(chrono::NaiveDate),
    /// Date is before the earliest known season start
    BeforeEarliest(chrono::NaiveDate),
}

/// The earliest standings start date across all seasons, if any.
pub fn earliest_start(seasons: &[SeasonBounds]) -> Option<chrono::NaiveDate> {
    seasons.iter().filter_map(|s| s.start()).min()
}

/// The latest standings end date across all seasons, if any.
pub fn latest_end(seasons: &[SeasonBounds]) -> Option<chrono::NaiveDate> {
    seasons.iter().filter_map(|s| s.end()).max()
}

/// The season that ended most recently before `date` (the previous season for
/// an offseason-gap date), if any.
pub fn season_ending_before(
    date: chrono::NaiveDate,
    seasons: &[SeasonBounds],
) -> Option<&SeasonBounds> {
    seasons
        .iter()
        .filter(|s| s.end().is_some_and(|end| end < date))
        .max_by_key(|s| s.end())
}

/// The season that starts soonest after `date` (the upcoming season for an
/// offseason-gap date), if any.
pub fn season_starting_after(
    date: chrono::NaiveDate,
    seasons: &[SeasonBounds],
) -> Option<&SeasonBounds> {
    seasons
        .iter()
        .filter(|s| s.start().is_some_and(|start| start > date))
        .min_by_key(|s| s.start())
}

/// Classify a requested date against the known seasons.
/// Returns `None` only if the season list is empty.
pub fn resolve_date(
    requested: chrono::NaiveDate,
    seasons: &[SeasonBounds],
) -> Option<DateResolution> {
    // In-season: use the requested date as-is.
    let in_season = seasons.iter().any(|s| match (s.start(), s.end()) {
        (Some(start), Some(end)) => requested >= start && requested <= end,
        _ => false,
    });
    if in_season {
        return Some(DateResolution::InSeason(requested));
    }

    let earliest = earliest_start(seasons)?;
    let latest = latest_end(seasons)?;

    if requested < earliest {
        return Some(DateResolution::BeforeEarliest(earliest));
    }
    if requested > latest {
        return Some(DateResolution::AfterLatest(latest));
    }

    // Offseason gap
    let prev_end = seasons
        .iter()
        .filter_map(|s| s.end())
        .filter(|&end| end < requested)
        .max()
        .unwrap_or(earliest);
    Some(DateResolution::OffseasonGap(prev_end))
}

/// Classification of a requested season end year relative to the known seasons.
/// Used by the year-based team stats page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YearResolution {
    /// Year is within the available range; fetch stats for it.
    InRange,
    /// Year is after the latest available season end year. Carries that year.
    AfterLatest(i32),
    /// Year is before the earliest available season end year. Carries that year.
    BeforeEarliest(i32),
}

/// The earliest season end year across all seasons, if any.
pub fn earliest_end_year(seasons: &[SeasonBounds]) -> Option<i32> {
    seasons.iter().map(|s| s.end_year()).min()
}

/// The latest season end year across all seasons, if any.
pub fn latest_end_year(seasons: &[SeasonBounds]) -> Option<i32> {
    seasons.iter().map(|s| s.end_year()).max()
}

/// Classify a requested season end year against the known seasons.
/// Returns `None` only if the season list is empty.
pub fn resolve_year(year: i32, seasons: &[SeasonBounds]) -> Option<YearResolution> {
    let earliest = earliest_end_year(seasons)?;
    let latest = latest_end_year(seasons)?;
    if year < earliest {
        Some(YearResolution::BeforeEarliest(earliest))
    } else if year > latest {
        Some(YearResolution::AfterLatest(latest))
    } else {
        Some(YearResolution::InRange)
    }
}

/// The message for a season end year outside the known seasons, or `None` if
/// it's in range (or the season list is empty). Shared by Team Stats and
/// Playoffs so both word it the same way.
pub fn year_out_of_range_message(year: i32, seasons: &[SeasonBounds]) -> Option<String> {
    match resolve_year(year, seasons)? {
        YearResolution::AfterLatest(latest) => Some(format!(
            "No stats for this season yet. Latest available: {}-{}.",
            latest - 1,
            latest
        )),
        YearResolution::BeforeEarliest(earliest) => Some(format!(
            "No stats for this season. Earliest available: {}-{}.",
            earliest - 1,
            earliest
        )),
        YearResolution::InRange => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn out_of_range_years_get_the_shared_message() {
        let seasons = vec![
            SeasonBounds {
                id: 19171918,
                standings_start: "1917-12-19".to_string(),
                standings_end: "1918-03-06".to_string(),
            },
            SeasonBounds {
                id: 20262027,
                standings_start: "2026-09-29".to_string(),
                standings_end: "2026-10-10".to_string(),
            },
        ];
        assert_eq!(
            year_out_of_range_message(1915, &seasons).as_deref(),
            Some("No stats for this season. Earliest available: 1917-1918.")
        );
        assert_eq!(
            year_out_of_range_message(2030, &seasons).as_deref(),
            Some("No stats for this season yet. Latest available: 2026-2027.")
        );
        assert_eq!(year_out_of_range_message(2005, &seasons), None);
        assert_eq!(year_out_of_range_message(2027, &seasons), None);
    }

    #[test]
    fn fallback_year_counts_september_onward_as_next_season() {
        assert_eq!(season_end_year_fallback(d("2026-09-01")), 2027);
        assert_eq!(season_end_year_fallback(d("2026-10-10")), 2027);
        assert_eq!(season_end_year_fallback(d("2026-12-31")), 2027);
        assert_eq!(season_end_year_fallback(d("2027-03-01")), 2027);
        // Summer still points at the season that just ended
        assert_eq!(season_end_year_fallback(d("2026-07-15")), 2026);
        assert_eq!(season_end_year_fallback(d("2026-08-31")), 2026);
    }

    fn season(start: &str, end: &str) -> SeasonBounds {
        // Derive the season id from the start year
        let start_year = NaiveDate::parse_from_str(start, "%Y-%m-%d")
            .unwrap()
            .format("%Y")
            .to_string()
            .parse::<u32>()
            .unwrap();
        let id = start_year * 10000 + (start_year + 1);
        SeasonBounds {
            id,
            standings_start: start.to_string(),
            standings_end: end.to_string(),
        }
    }

    fn seasons() -> Vec<SeasonBounds> {
        vec![
            season("2024-10-04", "2025-04-17"),
            season("2025-10-07", "2026-04-17"),
            season("2026-09-29", "2027-04-10"),
        ]
    }

    #[test]
    fn end_year_uses_season_id_not_standings_end_year() {
        let in_progress = vec![
            season("2025-10-07", "2026-04-17"),
            SeasonBounds {
                id: 20262027,
                standings_start: "2026-09-29".to_string(),
                standings_end: "2026-10-04".to_string(),
            },
        ];
        assert_eq!(
            season_end_year_for_date(d("2026-10-04"), &in_progress),
            Some(2027)
        );
    }

    #[test]
    fn end_year_within_season() {
        // Oct 29 2026 is within the 2026-2027 season -> end year 2027.
        assert_eq!(
            season_end_year_for_date(d("2026-10-29"), &seasons()),
            Some(2027)
        );
        // Mid 2025-2026 season.
        assert_eq!(
            season_end_year_for_date(d("2025-12-01"), &seasons()),
            Some(2026)
        );
    }

    #[test]
    fn end_year_offseason_uses_previous_season() {
        // Aug 24 2026 is between 2025-26 and 2026-27 -> previous season end 2026.
        assert_eq!(
            season_end_year_for_date(d("2026-08-24"), &seasons()),
            Some(2026)
        );
    }

    #[test]
    fn end_year_after_latest_and_before_first() {
        assert_eq!(
            season_end_year_for_date(d("2030-01-01"), &seasons()),
            Some(2027)
        );
        assert_eq!(
            season_end_year_for_date(d("2000-01-01"), &seasons()),
            Some(2025)
        );
    }

    #[test]
    fn end_year_empty_is_none() {
        assert_eq!(season_end_year_for_date(d("2026-10-29"), &[]), None);
    }

    #[test]
    fn resolve_in_season() {
        assert_eq!(
            resolve_date(d("2025-12-01"), &seasons()),
            Some(DateResolution::InSeason(d("2025-12-01")))
        );
        // Inclusive boundaries.
        assert_eq!(
            resolve_date(d("2026-09-29"), &seasons()),
            Some(DateResolution::InSeason(d("2026-09-29")))
        );
        assert_eq!(
            resolve_date(d("2027-04-10"), &seasons()),
            Some(DateResolution::InSeason(d("2027-04-10")))
        );
    }

    #[test]
    fn resolve_offseason_gap_uses_previous_end() {
        // Between 2025-26 (ends 04-17) and 2026-27 (starts 09-29).
        assert_eq!(
            resolve_date(d("2026-08-24"), &seasons()),
            Some(DateResolution::OffseasonGap(d("2026-04-17")))
        );
    }

    #[test]
    fn resolve_after_latest_is_out_of_range() {
        // Latest season ends 2027-04-10.
        assert_eq!(
            resolve_date(d("2300-10-01"), &seasons()),
            Some(DateResolution::AfterLatest(d("2027-04-10")))
        );
        // Day after the latest end is already out of range.
        assert_eq!(
            resolve_date(d("2027-04-11"), &seasons()),
            Some(DateResolution::AfterLatest(d("2027-04-10")))
        );
    }

    #[test]
    fn resolve_before_earliest_is_out_of_range() {
        // Earliest season starts 2024-10-04.
        assert_eq!(
            resolve_date(d("2000-01-01"), &seasons()),
            Some(DateResolution::BeforeEarliest(d("2024-10-04")))
        );
        assert_eq!(
            resolve_date(d("2024-10-03"), &seasons()),
            Some(DateResolution::BeforeEarliest(d("2024-10-04")))
        );
    }

    #[test]
    fn resolve_empty_is_none() {
        assert_eq!(resolve_date(d("2026-10-29"), &[]), None);
    }

    #[test]
    fn resolve_year_in_range() {
        // seasons() end years: 2025, 2026, 2027.
        assert_eq!(
            resolve_year(2025, &seasons()),
            Some(YearResolution::InRange)
        );
        assert_eq!(
            resolve_year(2026, &seasons()),
            Some(YearResolution::InRange)
        );
        assert_eq!(
            resolve_year(2027, &seasons()),
            Some(YearResolution::InRange)
        );
    }

    #[test]
    fn resolve_year_after_latest() {
        assert_eq!(
            resolve_year(2030, &seasons()),
            Some(YearResolution::AfterLatest(2027))
        );
        assert_eq!(
            resolve_year(2028, &seasons()),
            Some(YearResolution::AfterLatest(2027))
        );
    }

    #[test]
    fn resolve_year_before_earliest() {
        assert_eq!(
            resolve_year(1950, &seasons()),
            Some(YearResolution::BeforeEarliest(2025))
        );
        assert_eq!(
            resolve_year(2024, &seasons()),
            Some(YearResolution::BeforeEarliest(2025))
        );
    }

    #[test]
    fn resolve_year_empty_is_none() {
        assert_eq!(resolve_year(2026, &[]), None);
    }
}
