use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{AppEvent, Source, TabGate, send_request};
use crate::models::standings::{
    DateResolution, SeasonBounds, resolve_date, season_ending_before, season_starting_after,
};
use crate::sources::StandingsResponse;

/// Standings change at most once per game, so a fixed interval is plenty
const FETCH_INTERVAL: Duration = Duration::from_secs(60);

#[allow(clippy::enum_variant_names)] // Commands read as `StandingsCommand::SetX`
pub enum StandingsCommand {
    SetDate(String),
    SetSeasonBounds(Vec<SeasonBounds>),
    /// Whether this source's tab is shown
    SetActive(bool),
}

pub struct StandingsSource {
    client: reqwest::Client,
    rx: Receiver<StandingsCommand>,
    gate: TabGate,
    current_date: String,
    seasons: Option<Vec<SeasonBounds>>,
}
impl StandingsSource {
    pub fn new(
        client: reqwest::Client,
        rx: Receiver<StandingsCommand>,
        current_date: String,
    ) -> Self {
        Self {
            client,
            rx,
            gate: TabGate::default(),
            current_date,
            seasons: None,
        }
    }

    /// Resolve the requested date against the season bounds. Returns the
    /// classification, or `None` if bounds aren't available yet
    fn resolved(&self) -> Option<DateResolution> {
        let seasons = self.seasons.as_deref()?;
        let requested = chrono::NaiveDate::parse_from_str(&self.current_date, "%Y-%m-%d").ok()?;
        resolve_date(requested, seasons)
    }

    fn unavailable_message(&self) -> String {
        let fmt = |s: &SeasonBounds| -> String {
            match (s.start(), s.end()) {
                (Some(start), Some(end)) => format!(
                    "{} - {}",
                    start.format("%B %d, %Y"),
                    end.format("%B %d, %Y")
                ),
                // Fall back to the raw strings if either doesn't parse.
                _ => format!("{} - {}", s.standings_start, s.standings_end),
            }
        };

        let mut lines = vec!["No standings available.".to_string()];
        if let (Some(seasons), Ok(requested)) = (
            self.seasons.as_deref(),
            chrono::NaiveDate::parse_from_str(&self.current_date, "%Y-%m-%d"),
        ) {
            if let Some(prev) = season_ending_before(requested, seasons) {
                lines.push(format!("Previous season: {}.", fmt(prev)));
            }
            if let Some(next) = season_starting_after(requested, seasons) {
                lines.push(format!("Next season: {}.", fmt(next)));
            }
        }
        lines.join("\n")
    }

    async fn fetch(&mut self, tx: &Sender<AppEvent>) {
        let date = match self.resolved() {
            Some(DateResolution::InSeason(d)) => d.format("%Y-%m-%d").to_string(),
            Some(DateResolution::OffseasonGap(_)) => {
                // The requested date falls between two seasons
                let _ = tx
                    .send(AppEvent::StandingsOutOfRange {
                        date: self.current_date.clone(),
                        message: self.unavailable_message(),
                    })
                    .await;
                return;
            }
            Some(DateResolution::AfterLatest(latest)) => {
                let msg = format!(
                    "No standings for this date yet. Latest available: {}.",
                    latest.format("%B %d, %Y")
                );
                log::debug!("Requested standings date is after latest season: {}", msg);
                let _ = tx
                    .send(AppEvent::StandingsOutOfRange {
                        date: self.current_date.clone(),
                        message: msg,
                    })
                    .await;
                return;
            }
            Some(DateResolution::BeforeEarliest(earliest)) => {
                let msg = format!(
                    "No standings for this date. Earliest available: {}.",
                    earliest.format("%B %d, %Y")
                );
                log::debug!(
                    "Requested standings date is before earliest season: {}",
                    msg
                );
                let _ = tx
                    .send(AppEvent::StandingsOutOfRange {
                        date: self.current_date.clone(),
                        message: msg,
                    })
                    .await;
                return;
            }
            // Bounds unavailable or unparseable date: fetch the requested date
            None => self.current_date.clone(),
        };

        let season = self.matched_season(&date);
        let url = format!("https://api-web.nhle.com/v1/standings/{}", date);

        match send_request(self.client.get(&url)).await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    // Parse the JSON
                    match StandingsResponse::from_json(&body) {
                        Ok(parsed_standings) => {
                            log::debug!("Standings data successfully parsed (date {})", date);
                            let _ = tx
                                .send(AppEvent::StandingsUpdate {
                                    date: self.current_date.clone(),
                                    standings: parsed_standings,
                                    season,
                                })
                                .await;
                            log::debug!("Sent standings data to app");
                        }
                        Err(e) => log::error!("Failed to parse standings: {}", e),
                    }
                }
            }
            Err(err) => log::warn!("Failed to fetch standings: {}", err),
        }
    }

    /// The season bounds whose range contains `date` (the effective fetch date).
    fn matched_season(&self, date: &str) -> Option<SeasonBounds> {
        let seasons = self.seasons.as_deref()?;
        let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
        seasons
            .iter()
            .find(|s| match (s.start(), s.end()) {
                (Some(start), Some(end)) => d >= start && d <= end,
                _ => false,
            })
            .cloned()
    }
}

#[async_trait::async_trait]
impl Source for StandingsSource {
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(FETCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        StandingsCommand::SetActive(active) => {
                            if let Some(due) = self.gate.set_active(active, FETCH_INTERVAL) {
                                interval.reset_at(due);
                            }
                        }
                        StandingsCommand::SetDate(date) => {
                            self.current_date = date;
                            if self.gate.target_changed() {
                                self.fetch(&tx).await;
                                self.gate.fetched();
                                interval.reset();
                            }
                        }
                        StandingsCommand::SetSeasonBounds(seasons) => {
                            // Bounds are resolved exactly once by SeasonSource
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn season(start: &str, end: &str) -> SeasonBounds {
        let start_year = NaiveDate::parse_from_str(start, "%Y-%m-%d")
            .unwrap()
            .format("%Y")
            .to_string()
            .parse::<u32>()
            .unwrap();
        SeasonBounds {
            id: start_year * 10000 + (start_year + 1),
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

    // These assert the date-classification the source relies on to decide
    // whether to fetch (InSeason/OffseasonGap) or report out-of-range
    // (AfterLatest/BeforeEarliest). The resolution logic itself lives in
    // `models::standings::resolve_date`.

    #[test]
    fn in_season_fetches_requested_date() {
        assert_eq!(
            resolve_date(d("2025-12-01"), &seasons()),
            Some(DateResolution::InSeason(d("2025-12-01")))
        );
    }

    #[test]
    fn offseason_gap_resolves_to_previous_season_end() {
        // resolve_date still classifies gap dates as OffseasonGap; the standings
        // source now turns that into an out-of-range hint rather than fetching.
        assert_eq!(
            resolve_date(d("2026-08-24"), &seasons()),
            Some(DateResolution::OffseasonGap(d("2026-04-17")))
        );
    }

    #[test]
    fn surrounding_seasons_for_offseason_gap() {
        use crate::models::standings::{season_ending_before, season_starting_after};
        let s = seasons();
        // Aug 24 2026 is between 2025-26 (ends Apr 17 2026) and 2026-27
        // (starts Sep 29 2026).
        let prev = season_ending_before(d("2026-08-24"), &s).unwrap();
        let next = season_starting_after(d("2026-08-24"), &s).unwrap();
        assert_eq!(prev.standings_end, "2026-04-17");
        assert_eq!(next.standings_start, "2026-09-29");
    }

    #[test]
    fn surrounding_seasons_after_last_has_no_next() {
        use crate::models::standings::{season_ending_before, season_starting_after};
        let s = seasons();
        // A date after the final season: previous exists, next does not.
        assert!(season_ending_before(d("2027-06-01"), &s).is_some());
        assert!(season_starting_after(d("2027-06-01"), &s).is_none());
    }

    #[test]
    fn future_date_is_out_of_range() {
        assert_eq!(
            resolve_date(d("2300-10-01"), &seasons()),
            Some(DateResolution::AfterLatest(d("2027-04-10")))
        );
    }

    #[test]
    fn too_early_date_is_out_of_range() {
        assert_eq!(
            resolve_date(d("2000-01-01"), &seasons()),
            Some(DateResolution::BeforeEarliest(d("2024-10-04")))
        );
    }
}
