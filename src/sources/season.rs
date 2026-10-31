use chrono::NaiveDate;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{AppEvent, Source, send_request};
use crate::models::standings::{
    SeasonBounds, StandingsSeasonResponse, season_end_year_fallback, season_end_year_for_date,
};

/// How long to wait between retries if the season bounds can't be fetched.
const RETRY_INTERVAL: Duration = Duration::from_secs(30);

pub enum SeasonCommand {
    /// Today's game day changed
    SetToday(NaiveDate),
}

/// NHL season bounds. Re-fetched whenever the game day changes, since the
/// current season's standingsEnd is today's date.
pub struct SeasonSource {
    client: reqwest::Client,
    rx: Receiver<SeasonCommand>,
    today: NaiveDate,
}

impl SeasonSource {
    pub fn new(client: reqwest::Client, rx: Receiver<SeasonCommand>, today: NaiveDate) -> Self {
        Self { client, rx, today }
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
    async fn run(mut self: Box<Self>, tx: Sender<AppEvent>, cancel: CancellationToken) {
        // Bounds have been emitted at least once
        let mut resolved = false;
        // The latest fetch failed, or the game day changed since the last one
        let mut need_fetch = true;

        loop {
            if need_fetch {
                let seasons = tokio::select! {
                    _ = cancel.cancelled() => return,
                    seasons = self.try_fetch_bounds() => seasons,
                };
                match seasons {
                    Some(seasons) => {
                        self.emit_resolved(&tx, seasons).await;
                        resolved = true;
                        need_fetch = false;
                    }
                    // A failed refresh keeps the bounds already sent, so the
                    // fallback year is only needed before the first success
                    None if !resolved => {
                        let fallback = self.fallback_year();
                        log::warn!(
                            "Using fallback season end year {} while retrying season bounds",
                            fallback
                        );
                        let _ = tx.send(AppEvent::SeasonResolved { year: fallback }).await;
                    }
                    None => log::warn!("Season bounds refresh failed; retrying"),
                }
            }

            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(RETRY_INTERVAL), if need_fetch => {}
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        SeasonCommand::SetToday(date) => {
                            if date != self.today {
                                // The current season's standingsEnd tracks today, so
                                // bounds fetched yesterday end a day short
                                log::debug!("Game day changed to {}; refreshing season bounds", date);
                                self.today = date;
                                need_fetch = true;
                            }
                        }
                    }
                }
            }
        }
    }
}
