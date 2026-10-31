use crate::{
    sources::{
        games::{
            boxscore::BoxscoreCommand, game_story::GameStoryCommand, games::GamesCommand,
            play_by_play::PlaysCommand, total_goals::TotalGoalsCommand,
        },
        playoffs::{
            bracket::BracketCommand, bracket_series::BracketSeriesCommand, series::SeriesCommand,
        },
        season::SeasonCommand,
        standings::StandingsCommand,
        teams_stats::TeamStatsCommand,
    },
    state::{app_settings::AppSettings, app_state::AppState},
};
use chrono::Utc;
use tokio::sync::mpsc::Sender;

pub struct App {
    pub state: AppState,
    pub settings: AppSettings,
}

impl App {
    pub fn new(
        settings: AppSettings,
        games_tx: Sender<GamesCommand>,
        standings_tx: Sender<StandingsCommand>,
        boxscore_tx: Sender<BoxscoreCommand>,
        plays_tx: Sender<PlaysCommand>,
        game_story_tx: Sender<GameStoryCommand>,
        team_stats_tx: Sender<TeamStatsCommand>,
        bracket_tx: Sender<BracketCommand>,
        series_tx: Sender<SeriesCommand>,
        total_goals_tx: Sender<TotalGoalsCommand>,
        bracket_series_tx: Sender<BracketSeriesCommand>,
        season_tx: Sender<SeasonCommand>,
    ) -> Self {
        let mut app = Self {
            state: AppState::new(
                games_tx,
                standings_tx,
                boxscore_tx,
                plays_tx,
                game_story_tx,
                team_stats_tx,
                bracket_tx,
                series_tx,
                total_goals_tx,
                bracket_series_tx,
                season_tx,
            ),
            settings,
        };
        app.configure();
        app
    }

    /// Run any final configuration
    fn configure(&mut self) {
        self.set_all_datepickers_to_today();
        self.set_time_zone();

        // Log the resolved config once at startup
        log::info!(
            "Config: timezone={}, log_level={}, favorite_team={:?}",
            self.settings.timezone,
            self.settings.log_level.unwrap_or(log::LevelFilter::Error),
            self.settings.favorite_team,
        );

        // Apply the favorite team (if configured) as the default team shown on
        // the Team Stats page. The team stats source is initialized with the
        // same team in `main.rs`, so no command needs to be sent here.
        if let Some(favorite) = self.settings.favorite_team {
            self.state.team_stats.team_picker.current_team = favorite;
        }

        // Keep the in-app logger widget's level in sync with the config.
        if let Some(level) = self.settings.log_level {
            tui_logger::set_default_level(level);
        }
    }

    /// Sync date pickers using the correct timezone.
    fn set_all_datepickers_to_today(&mut self) {
        let today = Utc::now()
            .with_timezone(&self.settings.timezone)
            .date_naive();
        self.state.date_state.date = today;
        self.state.date_state.today = today;
    }

    /// Set the timezone in the state to the settings' timezone
    fn set_time_zone(&mut self) {
        self.state.timezone = self.settings.timezone;
    }
}
