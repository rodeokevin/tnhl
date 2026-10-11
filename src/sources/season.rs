use chrono::NaiveDate;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{AppEvent, Source, send_request};
use crate::models::standings::{
    SeasonBounds, StandingsSeasonResponse, season_end_year_fallback, season_end_year_for_date,
};

/// How long to wait between retries if the season bounds can't be fetched.
const RETRY_INTERVAL: Duration = Duration::from_secs(30);

/// NHL season bounds
pub struct SeasonSource {
    client: reqwest::Client,
    today: NaiveDate,
}

impl SeasonSource {
    pub fn new(client: reqwest::Client, today: NaiveDate) -> Self {
        Self { client, today }
    }

    /// Try to fetch and parse the season bounds
    async fn try_fetch_bounds(&self) -> Option<Vec<SeasonBounds>> {
        let url = "https://api-web.nhle.com/v1/standings-season";
        match send_request(self.client.get(url)).await {
            Ok(resp) => match resp.text().await {
                Ok(body) => match StandingsSeasonResponse::from_json(&body) {
                    Ok(parsed) if !parsed.seasons.is_empty() => Some(parsed.seasons),
                    Ok(_) => {
                        log::warn!("standings-season returned no seasons");
                        None
                    }
                    Err(e) => {
                        log::error!("Failed to parse standings-season: {}", e);
                        None
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read standings-season body: {}", e);
                    None
                }
            },
            Err(e) => {
                log::warn!("Failed to fetch standings-season: {}", e);
                None
            }
        }
    }

    /// Fallback season end year when the bounds are unavailable
    fn fallback_year(&self) -> i32 {
        season_end_year_fallback(self.today)
    }

    /// Emit the resolved bounds and the current season end year
    async fn emit_resolved(&self, tx: &Sender<AppEvent>, seasons: Vec<SeasonBounds>) {
        let year = season_end_year_for_date(self.today, &seasons).unwrap_or_else(|| {
            log::warn!("Could not resolve season year from bounds; using fallback");
            self.fallback_year()
        });
        log::debug!(
            "Resolved {} season bounds, current end year {}",
            seasons.len(),
            year
        );
        let _ = tx.send(AppEvent::SeasonBoundsResolved { seasons }).await;
        let _ = tx.send(AppEvent::SeasonResolved { year }).await;
    }
}

#[async_trait::async_trait]
impl Source for SeasonSource {
    async fn run(self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        // First attempt.
        if let Some(seasons) = tokio::select! {
            _ = cancel.cancelled() => return,
            seasons = self.try_fetch_bounds() => seasons,
        } {
            self.emit_resolved(&tx, seasons).await;
            return;
        }

        // Fetch failed
        let fallback = self.fallback_year();
        log::warn!(
            "Using fallback season end year {} while retrying season bounds",
            fallback
        );
        let _ = tx.send(AppEvent::SeasonResolved { year: fallback }).await;

        let mut interval = tokio::time::interval(RETRY_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await; // consume the immediate first tick
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    if let Some(seasons) = self.try_fetch_bounds().await {
                        self.emit_resolved(&tx, seasons).await;
                        break;
                    }
                }
            }
        }
    }
}
