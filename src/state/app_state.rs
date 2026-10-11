use std::fmt::Debug;

use crate::input::{Action, map_key};
use crate::models::games::games::{GameState, GamesResponse};
use crate::sources::playoffs::series::SeriesCommand;
use crate::sources::{
    AppEvent, FetchInterval,
    games::{
        boxscore::BoxscoreCommand,
        game_story::GameStoryCommand,
        games::GamesCommand,
        play_by_play::PlaysCommand,
        total_goals::{TotalGoalsCommand, TotalGoalsTarget},
    },
    playoffs::bracket::BracketCommand,
    playoffs::bracket_series::{BracketSeriesCommand, BracketSeriesTarget},
    standings::StandingsCommand,
    teams_stats::TeamStatsCommand,
};
use crate::state::playoffs_state::PlayoffsFocus;
use crate::state::team_stats::team_picker::InputError;
use crate::state::team_stats::team_stats_state::PlayerType;
use crate::state::{
    date_state::DateState, games_state::BoxscorePosition, games_state::GamesFocus,
    games_state::GamesState, help::HelpState, playoffs_state::PlayoffsState,
    standings_state::StandingsState, team_stats::team_stats_state::TeamStatsState,
};
use chrono::ParseError;
use chrono_tz::Tz;
use ratatui::widgets::TableState;
use tokio::sync::mpsc::Sender;

/// Which pane currently has keyboard focus.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PaneFocus {
    #[default]
    Content,
    DatePicker,
    /// Widget for selecting the team and year for team stats page
    TeamPicker,
    Help,
}

/// Which menu item is currently selected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MenuFocus {
    #[default]
    Games,
    Standings,
    TeamStats,
    Playoffs,
}

impl MenuFocus {
    pub fn index(&self) -> usize {
        match self {
            MenuFocus::Games => 0,
            MenuFocus::Standings => 1,
            MenuFocus::TeamStats => 2,
            MenuFocus::Playoffs => 3,
        }
    }
}

/// Which sources should be polling right now
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveSources {
    /// Scoreboard, game story and total goals
    games: bool,
    boxscore: bool,
    plays: bool,
    standings: bool,
    team_stats: bool,
    /// Bracket, series and bracket series
    playoffs: bool,
}

pub struct AppState {
    pub date_state: DateState,
    pub timezone: Tz,

    pub games_tx: Sender<GamesCommand>,
    pub standings_tx: Sender<StandingsCommand>,
    pub boxscore_tx: Sender<BoxscoreCommand>,
    pub plays_tx: Sender<PlaysCommand>,
    pub game_story_tx: Sender<GameStoryCommand>,
    pub team_stats_tx: Sender<TeamStatsCommand>,
    pub bracket_tx: Sender<BracketCommand>,
    pub series_tx: Sender<SeriesCommand>,
    pub total_goals_tx: Sender<TotalGoalsCommand>,
    pub bracket_series_tx: Sender<BracketSeriesCommand>,

    pub selected_menu: MenuFocus,
    pub display_menu: bool,

    pub standings: StandingsState,
    pub games: GamesState,
    pub team_stats: TeamStatsState,
    pub playoffs: PlayoffsState,

    pub help: HelpState,

    pub focus: PaneFocus,
    pub previous_focus: PaneFocus,
    pub should_quit: bool,
    /// Which sources were last told to poll, so changes are only sent once
    sent_active: Option<ActiveSources>,
}

impl AppState {
    pub fn new(
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
    ) -> Self {
        Self {
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

            date_state: DateState::default(),
            timezone: Tz::default(),

            selected_menu: MenuFocus::default(),
            display_menu: true,

            standings: StandingsState::default(),
            games: GamesState::default(),
            team_stats: TeamStatsState::default(),
            playoffs: PlayoffsState::default(),

            help: HelpState::default(),

            focus: PaneFocus::default(),
            previous_focus: PaneFocus::default(),
            should_quit: false,
            sent_active: None,
        }
    }
}

impl AppState {
    // Handle an incoming event and update state accordingly
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SeasonResolved { year } => {
                log::debug!("Season resolved to end year {}", year);
                // Only fill in the selected year while it's still unset (0): a
                // year the user picked, or one already filled in by an earlier
                // result (e.g. a fallback before a retry), is never replaced.
                // "Today" in the year picker always jumps to the latest result.
                self.date_state.current_season_year = Some(year);
                if self.date_state.year == 0 {
                    self.date_state.year = year;
                    // Fetch team stats / playoffs for the resolved season.
                    self.handle_year_change();
                }
            }
            AppEvent::TodayChanged { date } => {
                log::debug!("Today's game day is now {}", date);
                // Only `today` moves; the selected date stays where the user put it
                self.date_state.today = date;
                let live = self
                    .games
                    .games_data
                    .as_ref()
                    .is_some_and(|games| self.should_poll_fast_games(games));
                self.set_fetch_interval(live);
            }
            AppEvent::SeasonBoundsResolved { seasons } => {
                log::debug!("Season bounds resolved ({} seasons)", seasons.len());
                self.standings_tx
                    .try_send(StandingsCommand::SetSeasonBounds(seasons.clone()))
                    .ok();
                self.team_stats_tx
                    .try_send(TeamStatsCommand::SetSeasonBounds(seasons))
                    .ok();
            }
            AppEvent::StandingsUpdate { ref date, .. }
            | AppEvent::StandingsOutOfRange { ref date, .. }
                if !self.is_selected_date(Some(date)) =>
            {
                log::debug!("Ignoring standings for {}", date);
            }
            AppEvent::TeamStatsRegularSeasonUpdate { team, year, .. }
            | AppEvent::TeamStatsPlayoffsUpdate { team, year, .. }
            | AppEvent::TeamStatsOutOfRange { team, year, .. }
                if !self.is_selected_team_stats(team, year) =>
            {
                log::debug!("Ignoring team stats for {} {}", team, year);
            }
            AppEvent::BracketUpdate { year, .. } if year != self.date_state.year => {
                log::debug!("Ignoring bracket for {}", year);
            }
            AppEvent::StandingsUpdate {
                standings, season, ..
            } => {
                log::debug!("Updating standings data");
                self.standings.set_data(standings);
                self.standings.season = season;
                self.standings.out_of_range = None;
            }
            AppEvent::StandingsOutOfRange { message, .. } => {
                log::debug!("Standings date out of range: {}", message);
                // Clear stale data
                self.standings.standings_data = None;
                self.standings.season = None;
                self.standings.out_of_range = Some(message);
            }
            AppEvent::GamesUpdate { parsed_games }
                if !self.is_selected_date(parsed_games.current_date.as_deref()) =>
            {
                log::debug!("Ignoring games for {:?}", parsed_games.current_date);
            }
            AppEvent::GamesUnavailable { date, .. } if !self.is_selected_date(Some(&date)) => {
                log::debug!("Ignoring unavailable games for {}", date);
            }
            AppEvent::GamesUpdate { parsed_games } => {
                self.set_fetch_interval(self.should_poll_fast_games(&parsed_games));
                log::debug!("Updating games data");
                let total_goals_targets: Vec<TotalGoalsTarget> = parsed_games
                    .games
                    .iter()
                    .filter_map(|g| {
                        g.series_status.as_ref().and_then(|s| {
                            (s.needed_to_win == 0 && !s.series_letter.is_empty()).then(|| {
                                let start_year = g.id / 1_000_000;
                                TotalGoalsTarget {
                                    game_id: g.id,
                                    season: format!("{}{}", start_year, start_year + 1),
                                    letter: s.series_letter.clone(),
                                }
                            })
                        })
                    })
                    .collect();
                self.games.games_data = Some(parsed_games);
                self.games.unavailable = None;
                self.games.clamp_selected_game();
                self.sync_selected_game();
                self.total_goals_tx
                    .try_send(TotalGoalsCommand::SetTargets(total_goals_targets))
                    .ok();
            }
            AppEvent::GamesUnavailable { message, .. } => {
                // Keep games already shown for this date; a failed refresh
                // shouldn't replace them
                if self.games.games_data.is_none() {
                    log::debug!("Games unavailable: {}", message);
                    self.games.games_data = Some(GamesResponse {
                        prev_date: None,
                        current_date: None,
                        next_date: None,
                        games: Vec::new(),
                    });
                    self.games.unavailable = Some(message);
                    self.sync_selected_game();
                }
            }
            AppEvent::BoxscoreUpdate {
                game_id,
                parsed_boxscore,
            } => {
                log::debug!("Updating boxscore data for game {}", game_id);
                self.games.boxscore_data.insert(game_id, parsed_boxscore);
            }
            AppEvent::PlaysUpdate {
                game_id,
                parsed_plays,
            } => {
                log::debug!("Updating plays data for game {}", game_id);
                self.games.plays_data.insert(game_id, parsed_plays);
            }
            AppEvent::GameStoryUpdate {
                game_id,
                parsed_game_story,
            } => {
                log::debug!("Updating game story data for game {}", game_id);
                self.games
                    .game_story_data
                    .insert(game_id, parsed_game_story);
            }
            AppEvent::TeamStatsRegularSeasonUpdate {
                stats: parsed_team_stats,
                ..
            } => {
                log::debug!("Updating regular season team stats data");
                self.team_stats.regular_season_team_stats_data = Some(parsed_team_stats);
                self.team_stats.out_of_range = None;
            }
            AppEvent::TeamStatsPlayoffsUpdate {
                stats: parsed_team_stats,
                ..
            } => {
                log::debug!("Updating playoffs team stats data");
                self.team_stats.playoffs_team_stats_data = Some(parsed_team_stats);
                self.team_stats.out_of_range = None;
            }
            AppEvent::TeamStatsOutOfRange { message, .. } => {
                log::debug!("Team stats year out of range: {}", message);
                // Clear stale data so we don't show stats for a different season
                // than the one requested.
                self.team_stats.regular_season_team_stats_data = None;
                self.team_stats.playoffs_team_stats_data = None;
                self.team_stats.out_of_range = Some(message);
            }
            AppEvent::BracketUpdate {
                bracket: parsed_bracket,
                ..
            } => {
                log::debug!("Updating playoff bracket data");
                // Two-game total-goals series last used in 1936
                const LAST_TOTAL_GOALS_YEAR: i32 = 1936;
                let bracket_targets: Vec<BracketSeriesTarget> =
                    if self.date_state.year <= LAST_TOTAL_GOALS_YEAR {
                        let season = self.selected_season_id();
                        parsed_bracket
                            .series
                            .iter()
                            .filter(|s| !s.series_letter.is_empty())
                            .map(|s| BracketSeriesTarget {
                                season: season.clone(),
                                letter: s.series_letter.clone(),
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                self.playoffs.bracket_data = Some(parsed_bracket);
                self.bracket_series_tx
                    .try_send(BracketSeriesCommand::SetTargets(bracket_targets))
                    .ok();
            }
            AppEvent::SeriesUpdate {
                year,
                parsed_series,
            } => {
                // Drop responses for a series that's no longer open, including
                // the same letter from a previously selected year
                let selected = self.playoffs.selected_series.map(|c| c.to_string());
                if year == self.date_state.year
                    && selected.as_deref() == Some(parsed_series.series_letter.as_str())
                {
                    log::debug!("Updating series data");
                    self.playoffs.series_data = Some(parsed_series);
                } else {
                    log::debug!(
                        "Ignoring series {} for {}",
                        parsed_series.series_letter,
                        year
                    );
                }
            }
            AppEvent::TotalGoalsUpdate {
                game_id,
                parsed_series,
            } => {
                log::debug!("Updating Games total-goals series for game {}", game_id);
                self.games.total_goals_data.insert(game_id, parsed_series);
            }
            AppEvent::BracketSeriesUpdate {
                season,
                letter,
                parsed_series,
            } => {
                // Letters repeat every year, so drop results for another year
                if season == self.selected_season_id() {
                    log::debug!("Updating bracket series for {}", letter);
                    self.playoffs
                        .bracket_series_data
                        .insert(letter, parsed_series);
                } else {
                    log::debug!("Ignoring bracket series {} for {}", letter, season);
                }
            }
            AppEvent::Input(key_event) => {
                log::trace!("Key event detected: {:?}", key_event);
                let action = map_key(key_event, self);
                self.handle_action(action);
            }
            AppEvent::Tick => {
                self.games.sweeping_status_offset =
                    self.games.sweeping_status_offset.wrapping_add(1);
            }
        }
        // The Games view can also change while rendering (pregame -> live), so
        // check after every event, ticks included
        self.sync_active_sources();
    }

    /// Handle actions mapped from key events
    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,

            Action::ToggleDisplayMenu => self.display_menu = !self.display_menu,

            Action::DatePickerInputChar(c) => {
                self.date_state.is_valid = true; // reset status
                self.date_state.text.push(c);
            }
            Action::TeamPickerInputChar(c) => {
                self.team_stats.team_picker.is_valid = true; // reset status
                self.team_stats.team_picker.text.push(c);
            }
            Action::SelectMenu(i) => {
                let prev = self.selected_menu;
                self.selected_menu = self.select_menu(i);
                if prev != self.selected_menu {
                    self.reset_app_state();
                }
            }
            Action::PrevGame => {
                let prev = self.games.selected_game_index;
                self.games.shift_game_index(false);
                if self.games.selected_game_index != prev {
                    self.games.reset_game_state();
                    self.sync_selected_game();
                }
            }
            Action::NextGame => {
                let prev = self.games.selected_game_index;
                self.games.shift_game_index(true);
                if self.games.selected_game_index != prev {
                    self.games.reset_game_state();
                    self.sync_selected_game();
                }
            }
            Action::PrevGamesDisplay => {
                self.games.cycle_display(false);
                self.games.reset_scoring_scroll();
                self.games.reset_boxscore_state();
            }
            Action::NextGamesDisplay => {
                self.games.cycle_display(true);
                self.games.reset_scoring_scroll();
                self.games.reset_boxscore_state();
            }
            Action::GamesPageUp => self.games.games_page_up(),
            Action::GamesPageDown => self.games.games_page_down(),
            Action::GamesScrollUp => {
                self.games.scroll_offset = self.games.scroll_offset.saturating_sub(1);
            }
            Action::GamesScrollDown => {
                self.games.scroll_offset = self
                    .games
                    .scroll_offset
                    .saturating_add(1)
                    .min(self.games.max_scroll);
            }
            Action::TogglePlays => self.games.toggle_plays(),
            Action::ToggleGamesPane => self.games.toggle_plays_focus(),
            Action::PlaysScrollUp => self.games.plays_scroll_up(),
            Action::PlaysScrollDown => self.games.plays_scroll_down(),
            Action::PlaysPageUp => self.games.plays_page_up(),
            Action::PlaysPageDown => self.games.plays_page_down(),
            Action::BoxscorePageUp => self.games.boxscore_page_up(),
            Action::BoxscorePageDown => self.games.boxscore_page_down(),
            Action::BoxscoreUp => self.games.boxscore_row_up(),
            Action::BoxscoreDown => self.games.boxscore_row_down(),
            Action::BoxscoreForwards => {
                self.games.boxscore_table_state.select(Some(0));
                self.games.boxscore_selected_position = BoxscorePosition::Forwards
            }
            Action::BoxscoreDefensemen => {
                self.games.boxscore_table_state.select(Some(0));
                self.games.boxscore_selected_position = BoxscorePosition::Defensemen
            }
            Action::BoxscoreGoalies => {
                self.games.boxscore_table_state.select(Some(0));
                self.games.boxscore_selected_position = BoxscorePosition::Goalies
            }
            Action::BoxscoreToggleTeam => {
                self.games.boxscore_table_state.select(Some(0));
                self.games.boxscore_selected_position = BoxscorePosition::default();
                self.games.boxscore_selected_team = self.games.boxscore_selected_team.toggle()
            }
            // Standings actions
            Action::StandingsUp => self.standings.row_up(),
            Action::StandingsDown => self.standings.row_down(),
            Action::StandingsPageUp => self.standings.page_up(),
            Action::StandingsPageDown => self.standings.page_down(),
            Action::StandingsLeft => {
                if self.standings.shift_standings_type(false) {
                    self.standings.reset_table_state();
                }
            }
            Action::StandingsRight => {
                if self.standings.shift_standings_type(true) {
                    self.standings.reset_table_state();
                }
            }
            Action::PrevStandingsDisplay => {
                if self.standings.cycle_display(false) {
                    self.standings.reset_table_state();
                }
            }
            Action::NextStandingsDisplay => {
                if self.standings.cycle_display(true) {
                    self.standings.reset_table_state();
                }
            }
            // Team stats page actions
            Action::TeamStatsUp => self.team_stats.row_up(),
            Action::TeamStatsDown => self.team_stats.row_down(),
            Action::TeamStatsPageUp => self.team_stats.page_up(),
            Action::TeamStatsPageDown => self.team_stats.page_down(),
            Action::TeamStatsSkaters => {
                self.team_stats.table_state.select(Some(0));
                self.team_stats.player_type = PlayerType::Skaters;
            }
            Action::TeamStatsGoalies => {
                self.team_stats.table_state.select(Some(0));
                self.team_stats.player_type = PlayerType::Goalies;
            }
            Action::ToggleTeamStatsGame => {
                self.team_stats.table_state.select(Some(0));
                self.team_stats.game_type = self.team_stats.game_type.toggle()
            }

            // Playoffs page actions
            Action::PlayoffsScrollUp => self.playoffs.scroll_up(),
            Action::PlayoffsScrollDown => self.playoffs.scroll_down(),
            Action::PlayoffsScrollLeft => self.playoffs.scroll_left(),
            Action::PlayoffsScrollRight => self.playoffs.scroll_right(),
            Action::PlayoffsPageUp => self.playoffs.page_up(),
            Action::PlayoffsPageDown => self.playoffs.page_down(),
            Action::PlayoffsPageLeft => self.playoffs.page_left(),
            Action::PlayoffsPageRight => self.playoffs.page_right(),
            Action::SelectSeries(letter) => {
                if self.playoffs.try_select_series(letter) {
                    self.handle_series_selection();
                    self.playoffs.focus = PlayoffsFocus::Series;
                }
            }
            Action::ExitSeries => {
                self.playoffs.exit_series();
                self.playoffs.series_data = None;
                self.playoffs.focus = PlayoffsFocus::Bracket;
                self.series_tx.try_send(SeriesCommand::SetSeries(None)).ok();
            }

            // Date/year picker actions
            Action::EnterDatePicker => {
                self.previous_focus = self.focus;
                self.focus = PaneFocus::DatePicker;
                self.date_state.text.clear();
            }
            Action::DateLeft => self.date_state.move_date_selector_by_arrow(false),
            Action::DateRight => self.date_state.move_date_selector_by_arrow(true),
            Action::DateBackspace => {
                self.date_state.text.pop();
            }
            Action::ExitDatePicker => {
                self.date_state.text.clear();
                self.date_state.date_selection_offset = 0;
                self.date_state.year_selection_offset = 0;
                self.date_state.is_valid = true;
                self.focus = self.previous_focus;
            }
            Action::UpdateDate => {
                let prev = self.date_state.date;
                if self.try_update_date_from_input().is_ok() {
                    if self.date_state.date != prev {
                        self.handle_date_change();
                    }
                    self.focus = self.previous_focus;
                }
            }
            Action::YearLeft => self.date_state.move_year_selector_by_arrow(false),
            Action::YearRight => self.date_state.move_year_selector_by_arrow(true),
            Action::UpdateYear => {
                let prev = self.date_state.year;
                if self.try_update_year_from_input().is_ok() {
                    if self.date_state.year != prev {
                        self.handle_year_change();
                    }
                    self.focus = self.previous_focus;
                }
            }
            // Team picker actions
            Action::EnterTeamPicker => {
                self.previous_focus = self.focus;
                self.focus = PaneFocus::TeamPicker;
                self.team_stats.team_picker.text.clear();
            }
            Action::TeamBackspace => {
                self.team_stats.team_picker.text.pop();
            }
            Action::ExitTeamPicker => {
                self.team_stats.team_picker.text.clear();
                self.team_stats.team_picker.is_valid = true;
                self.focus = self.previous_focus;
            }
            Action::UpdateTeam => {
                if self.try_update_team_from_input().is_ok() {
                    self.handle_team_change();
                    self.focus = self.previous_focus;
                }
            }
            // Help page actions
            Action::EnterHelp => {
                self.previous_focus = self.focus;
                self.focus = PaneFocus::Help;
            }
            Action::HelpPageUp => self.help.page_up(),
            Action::HelpPageDown => self.help.page_down(),
            Action::HelpScrollUp => self.help.row_up(),
            Action::HelpScrollDown => self.help.row_down(),
            Action::ExitHelp => {
                self.focus = self.previous_focus;
                self.help.reset();
            }

            Action::None => {}
        }
    }

    // Helper functions for handling actions
    fn select_menu(&mut self, index: usize) -> MenuFocus {
        match index {
            1 => MenuFocus::Games,
            2 => MenuFocus::Standings,
            3 => MenuFocus::TeamStats,
            4 => MenuFocus::Playoffs,
            _ => self.selected_menu,
        }
    }
    fn try_update_date_from_input(&mut self) -> Result<(), ParseError> {
        let valid_date = self.date_state.validate_input_date()?;
        self.date_state.set_date_from_valid_input(valid_date);
        Ok(())
    }
    /// Update data from sources after date change
    pub fn handle_date_change(&mut self) {
        let date = self.date_state.date.to_string();
        let games_res = self.games_tx.try_send(GamesCommand::SetDate(date.clone()));
        let standings_res = self
            .standings_tx
            .try_send(StandingsCommand::SetDate(date.clone()));

        if let Err(e) = &games_res {
            log::error!("Failed to send GamesCommand::SetDate: {:?}", e);
        } else {
            // Clear current data and reset all state in games since new data is
            // incoming.
            self.games.games_data = None;
            self.games.boxscore_data.clear();
            self.games.game_story_data.clear();
            self.games.plays_data.clear();
            self.games.unavailable = None;
            self.games.reset_state();
            self.sync_selected_game();
        }
        if let Err(e) = &standings_res {
            log::error!("Failed to send StandingsCommand::SetDate: {:?}", e);
        } else {
            // Clear current data/hint and reset state since new data is incoming
            self.standings.standings_data = None;
            self.standings.out_of_range = None;
            self.standings.reset_state();
        }
    }
    fn try_update_year_from_input(&mut self) -> Result<(), ()> {
        let valid_year = self.date_state.validate_input_year(self.timezone)?;
        self.date_state.set_year_from_valid_input(valid_year);
        Ok(())
    }
    /// Update data from sources after year change
    pub fn handle_year_change(&mut self) {
        let year = self.date_state.year;
        let bracket_res = self.bracket_tx.try_send(BracketCommand::SetYear(year));
        let series_res = self.series_tx.try_send(SeriesCommand::SetYear(year));
        let team_stats_res = self.team_stats_tx.try_send(TeamStatsCommand::SetYear(year));

        if let Err(e) = &bracket_res {
            log::error!("Failed to send BracketCommand::SetYear: {:?}", e);
        } else {
            // Clear old data and reset state
            self.playoffs.bracket_data = None;
            self.playoffs.series_data = None;
            // Keyed by series letter, which repeats every year
            self.playoffs.bracket_series_data.clear();
            self.playoffs.reset_state();
        }
        if let Err(e) = &series_res {
            log::error!("Failed to send SeriesCommand::SetYear: {:?}", e);
        }
        if let Err(e) = &team_stats_res {
            log::error!("Failed to send TeamStatsCommand::SetYear: {:?}", e);
        } else {
            // Clear old data and reset state
            self.team_stats.regular_season_team_stats_data = None;
            self.team_stats.playoffs_team_stats_data = None;
            self.team_stats.out_of_range = None;
            self.team_stats.reset_state();
        }
    }
    /// Update the selected team for team stats
    fn try_update_team_from_input(&mut self) -> Result<(), InputError> {
        let valid_team = self.team_stats.team_picker.validate_input()?;
        self.team_stats
            .team_picker
            .set_team_from_valid_input(valid_team);
        Ok(())
    }
    /// Update data from team stats sources after team change
    pub fn handle_team_change(&mut self) {
        let team = self.team_stats.team_picker.current_team;
        let res = self.team_stats_tx.try_send(TeamStatsCommand::SetTeam(team));

        if let Err(e) = &res {
            log::error!("Failed to send TeamStatsCommand::SetTeam: {:?}", e);
        } else {
            // Clear old data/hint since new data is incoming for the new team
            self.team_stats.regular_season_team_stats_data = None;
            self.team_stats.playoffs_team_stats_data = None;
            self.team_stats.out_of_range = None;
            self.team_stats.reset_state();
        }
    }
    /// Update series data
    pub fn handle_series_selection(&mut self) {
        let series = self.playoffs.selected_series;
        let res = self.series_tx.try_send(SeriesCommand::SetSeries(series));

        if let Err(e) = &res {
            log::error!("Failed to send SeriesCommand::SetSeries: {:?}", e);
        } else {
            self.playoffs.enter_series();
        }
    }

    /// Which sources should poll: those feeding the current tab, and for
    /// boxscore and play-by-play, only while their view is open
    fn active_sources(&self) -> ActiveSources {
        let tab = self.selected_menu;
        let games = tab == MenuFocus::Games;
        let focus = self.games.focus;
        ActiveSources {
            games,
            boxscore: games && focus == GamesFocus::Boxscore,
            // The plays pane isn't drawn on pregame
            plays: games && self.games.plays_visible && focus != GamesFocus::Pregame,
            standings: tab == MenuFocus::Standings,
            team_stats: tab == MenuFocus::TeamStats,
            playoffs: tab == MenuFocus::Playoffs,
        }
    }

    /// Tell sources whether to poll when that changes. A source whose data
    /// went stale while hidden refetches when it's shown again.
    pub fn sync_active_sources(&mut self) {
        let active = self.active_sources();
        if self.sent_active == Some(active) {
            return;
        }
        let sent = [
            self.games_tx
                .try_send(GamesCommand::SetActive(active.games))
                .is_ok(),
            self.game_story_tx
                .try_send(GameStoryCommand::SetActive(active.games))
                .is_ok(),
            self.boxscore_tx
                .try_send(BoxscoreCommand::SetActive(active.boxscore))
                .is_ok(),
            self.plays_tx
                .try_send(PlaysCommand::SetActive(active.plays))
                .is_ok(),
            self.total_goals_tx
                .try_send(TotalGoalsCommand::SetActive(active.games))
                .is_ok(),
            self.standings_tx
                .try_send(StandingsCommand::SetActive(active.standings))
                .is_ok(),
            self.team_stats_tx
                .try_send(TeamStatsCommand::SetActive(active.team_stats))
                .is_ok(),
            self.bracket_tx
                .try_send(BracketCommand::SetActive(active.playoffs))
                .is_ok(),
            self.series_tx
                .try_send(SeriesCommand::SetActive(active.playoffs))
                .is_ok(),
            self.bracket_series_tx
                .try_send(BracketSeriesCommand::SetActive(active.playoffs))
                .is_ok(),
        ];
        // If a channel was full, try again after the next event
        if sent.iter().all(|&ok| ok) {
            self.sent_active = Some(active);
        }
    }

    /// Point the per-game sources at the selected game
    fn sync_selected_game(&self) {
        let game = self.games.selected_game().map(|g| (g.id, g.game_state));
        self.game_story_tx
            .try_send(GameStoryCommand::SetGame(game))
            .ok();
        self.boxscore_tx
            .try_send(BoxscoreCommand::SetGame(game))
            .ok();
        self.plays_tx.try_send(PlaysCommand::SetGame(game)).ok();
    }

    fn reset_app_state(&mut self) {
        self.games.reset_state();
        self.standings.reset_state();
        self.team_stats.reset_state();
        self.playoffs.reset_state();
        // The selected game and series were reset, so point the sources at
        // them too
        self.sync_selected_game();
        self.series_tx.try_send(SeriesCommand::SetSeries(None)).ok();
    }

    /// Whether a team stats result fetched for `team` and `year` matches the
    /// current Team Stats selection
    fn is_selected_team_stats(&self, team: crate::models::TeamAbbrev, year: i32) -> bool {
        team == self.team_stats.team_picker.current_team && year == self.date_state.year
    }

    /// The selected season as the API's id, e.g. "20262027"
    fn selected_season_id(&self) -> String {
        format!("{}{}", self.date_state.year - 1, self.date_state.year)
    }

    /// Whether a response for `date` (YYYY-MM-DD) belongs to the selected
    /// date. Responses without a date are assumed current.
    fn is_selected_date(&self, date: Option<&str>) -> bool {
        date.is_none_or(|d| d == self.date_state.date.to_string())
    }

    /// Scoreboard interval: fast only for today, and faster while games are on.
    /// The other sources use fixed intervals or pick their own from the
    /// selected game's state.
    fn set_fetch_interval(&self, live: bool) {
        let interval = if self.date_state.date != self.date_state.today {
            // Only today's scoreboard changes quickly; other dates' rarely do
            FetchInterval::InfoLongInterval
        } else if live {
            FetchInterval::GamesShortInterval
        } else {
            FetchInterval::GamesLongInterval
        };
        self.games_tx
            .try_send(GamesCommand::SetInterval(interval.as_duration()))
            .ok();
    }

    /// If games are in the PRE/LIVE/CRIT states, we should use the short fetch interval
    fn should_poll_fast_games(&self, parsed_games: &GamesResponse) -> bool {
        parsed_games.games.iter().any(|g| {
            matches!(
                g.game_state,
                GameState::LIVE | GameState::CRIT | GameState::PRE
            )
        })
    }
}

pub fn table_page_up(visible_rows: usize, table_state: &mut TableState) {
    if visible_rows != 0 {
        // Page up so the current first visible row becomes the last visible row
        // of the new page, and select that row.
        let offset = table_state.offset();
        let new_offset = offset.saturating_sub(visible_rows - 1);
        *table_state.offset_mut() = new_offset;
        // The old top row (`offset`) is always within the new page; select it.
        table_state.select(Some(offset));
    }
}
pub fn table_page_down(visible_rows: usize, len: usize, table_state: &mut TableState) {
    // Nothing to page through in an empty table
    if visible_rows != 0 && len != 0 {
        // The last visible row becomes the first visible row
        // But if last visible row is the last row in the table, simply select it without changing the offset
        let offset = table_state.offset();
        let last_visible = if offset + visible_rows > len - 1 {
            len - 1
        } else {
            *table_state.offset_mut() = (offset + visible_rows - 1).min(len - 1);
            (offset + visible_rows - 1).min(len - 1)
        };
        table_state.select(Some(last_visible));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::FetchInterval;

    fn state_at(offset: usize, selected: usize) -> TableState {
        let mut t = TableState::default();
        *t.offset_mut() = offset;
        t.select(Some(selected));
        t
    }

    #[test]
    fn page_up_selects_old_top_row() {
        // Viewing rows 10..20 (10 visible). Page up should scroll so the old
        // top row (10) becomes the bottom of the new page, and select it.
        let mut ts = state_at(10, 15);
        table_page_up(10, &mut ts);
        assert_eq!(ts.offset(), 1); // 10 - (10 - 1)
        assert_eq!(ts.selected(), Some(10));
    }

    #[test]
    fn page_up_clamps_at_top() {
        let mut ts = state_at(3, 5);
        table_page_up(10, &mut ts);
        assert_eq!(ts.offset(), 0);
        assert_eq!(ts.selected(), Some(3)); // old top still within the new page
    }

    #[test]
    fn page_down_selects_old_bottom_row() {
        // Viewing rows 0..10 of a 50-row table; last visible row (9) becomes
        // the top of the new page and is selected.
        let mut ts = state_at(0, 4);
        table_page_down(10, 50, &mut ts);
        assert_eq!(ts.selected(), Some(9));
    }

    #[test]
    fn page_down_clamps_at_last_row() {
        let mut ts = state_at(45, 47);
        table_page_down(10, 50, &mut ts);
        assert_eq!(ts.selected(), Some(49)); // len - 1
    }

    #[test]
    fn page_down_on_empty_table_does_nothing() {
        let mut ts = state_at(0, 0);
        table_page_down(10, 0, &mut ts);
        assert_eq!(ts.selected(), Some(0));
        assert_eq!(ts.offset(), 0);
    }

    /// State with throwaway channels, except the scoreboard's
    fn test_state(games_tx: Sender<GamesCommand>) -> AppState {
        use tokio::sync::mpsc::channel;
        AppState::new(
            games_tx,
            channel(8).0,
            channel(8).0,
            channel(8).0,
            channel(8).0,
            channel(8).0,
            channel(8).0,
            channel(8).0,
            channel(8).0,
            channel(8).0,
        )
    }

    #[test]
    fn boxscore_and_plays_poll_only_while_their_view_is_open() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        let active = state.active_sources();
        assert!(active.games && !active.boxscore && !active.plays);

        state.games.focus = GamesFocus::Boxscore;
        assert!(state.active_sources().boxscore);

        state.games.focus = GamesFocus::Scoring;
        state.games.plays_visible = true;
        let active = state.active_sources();
        assert!(active.plays && !active.boxscore);

        // The plays pane isn't drawn on pregame
        state.games.focus = GamesFocus::Pregame;
        assert!(!state.active_sources().plays);

        // Nothing on the Games tab polls from another tab
        state.games.focus = GamesFocus::Boxscore;
        state.selected_menu = MenuFocus::Standings;
        let active = state.active_sources();
        assert!(!active.games && !active.boxscore && !active.plays && active.standings);
    }

    #[test]
    fn today_changed_moves_only_today_and_slows_old_scoreboard() {
        use tokio::sync::mpsc::channel;
        let (games_tx, mut games_rx) = channel(8);
        let mut state = test_state(games_tx);
        let old_today = chrono::NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let new_today = old_today.succ_opt().unwrap();
        state.date_state.date = old_today;
        state.date_state.today = old_today;

        state.handle_event(AppEvent::TodayChanged { date: new_today });

        assert_eq!(state.date_state.today, new_today);
        // The selected date is left alone
        assert_eq!(state.date_state.date, old_today);
        // The old day is no longer today, so its scoreboard polls slowly
        match games_rx.try_recv() {
            Ok(GamesCommand::SetInterval(interval)) => {
                assert_eq!(interval, FetchInterval::InfoLongInterval.as_duration())
            }
            _ => panic!("expected a scoreboard interval update"),
        }
    }

    fn unavailable(state: &AppState) -> AppEvent {
        AppEvent::GamesUnavailable {
            date: state.date_state.date.to_string(),
            message: "No game data available for this date.".to_string(),
        }
    }

    fn games_on(date: &str) -> GamesResponse {
        GamesResponse {
            prev_date: None,
            current_date: Some(date.to_string()),
            next_date: None,
            games: Vec::new(),
        }
    }

    #[test]
    fn games_unavailable_shows_message_instead_of_loading() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.games.games_data = None;
        state.handle_event(unavailable(&state));

        let data = state
            .games
            .games_data
            .as_ref()
            .expect("falls back to no games");
        assert!(data.games.is_empty());
        assert_eq!(
            state.games.unavailable.as_deref(),
            Some("No game data available for this date.")
        );
    }

    #[test]
    fn games_unavailable_keeps_already_loaded_games() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.games.games_data = Some(GamesResponse {
            prev_date: Some("2026-10-09".to_string()),
            current_date: Some("2026-10-10".to_string()),
            next_date: None,
            games: Vec::new(),
        });
        state.handle_event(unavailable(&state));

        let data = state.games.games_data.as_ref().unwrap();
        assert_eq!(data.current_date.as_deref(), Some("2026-10-10"));
        assert_eq!(state.games.unavailable, None);
    }

    #[test]
    fn games_for_another_date_are_ignored() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.games.games_data = None;
        state.handle_event(AppEvent::GamesUpdate {
            parsed_games: games_on("1999-01-01"),
        });
        state.handle_event(AppEvent::GamesUnavailable {
            date: "1999-01-01".to_string(),
            message: "stale".to_string(),
        });
        assert!(state.games.games_data.is_none());
        assert_eq!(state.games.unavailable, None);

        let today = state.date_state.date.to_string();
        state.handle_event(AppEvent::GamesUpdate {
            parsed_games: games_on(&today),
        });
        assert!(state.games.games_data.is_some());
    }

    #[test]
    fn unavailable_message_survives_tab_switch() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.games.games_data = None;
        state.handle_event(unavailable(&state));
        state.reset_app_state();
        assert!(state.games.unavailable.is_some());
    }

    #[test]
    fn first_season_resolution_sets_selected_year() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.handle_event(AppEvent::SeasonResolved { year: 2027 });
        assert_eq!(state.date_state.year, 2027);
        assert_eq!(state.date_state.current_season_year, Some(2027));
    }

    #[test]
    fn retried_season_resolution_does_not_change_selected_year() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        // Fallback year arrives first and is applied
        state.handle_event(AppEvent::SeasonResolved { year: 2026 });

        // The real season arrives after a retry: only "today" moves
        state.handle_event(AppEvent::SeasonResolved { year: 2027 });
        assert_eq!(state.date_state.year, 2026);
        assert_eq!(state.date_state.current_season_year, Some(2027));

        // Same when the user has picked another year in between
        state.date_state.year = 1995;
        state.handle_event(AppEvent::SeasonResolved { year: 2028 });
        assert_eq!(state.date_state.year, 1995);
    }

    #[test]
    fn slow_first_season_resolution_keeps_user_picked_year() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        assert_eq!(state.date_state.year, 0, "unset until resolved or picked");

        // The user picks a year before the first lookup returns
        state.date_state.set_year_from_valid_input(1995);
        state.handle_event(AppEvent::SeasonResolved { year: 2027 });
        assert_eq!(state.date_state.year, 1995);
        assert_eq!(state.date_state.current_season_year, Some(2027));
    }

    fn series(letter: &str) -> crate::models::playoffs::series::SeriesResponse {
        let seed = r#"{"id":1,"name":{"default":"A"},"abbrev":"MTL","placeName":{"default":"A"},"record":"0-0","seriesWins":0,"seed":1}"#;
        let json = format!(
            r#"{{"round":1,"roundAbbrev":"R1","roundLabel":"1st Round","seriesLetter":"{letter}","neededToWin":4,"length":7,"bottomSeedTeam":{seed},"topSeedTeam":{seed},"games":[]}}"#
        );
        crate::models::playoffs::series::SeriesResponse::from_json(&json).unwrap()
    }

    #[test]
    fn series_update_requires_open_series_and_selected_year() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.date_state.year = 1930;
        state.playoffs.selected_series = Some('A');

        // Same letter, previously selected year: ignored
        state.handle_event(AppEvent::SeriesUpdate {
            year: 1929,
            parsed_series: series("A"),
        });
        assert!(state.playoffs.series_data.is_none());

        // Another series: ignored
        state.handle_event(AppEvent::SeriesUpdate {
            year: 1930,
            parsed_series: series("B"),
        });
        assert!(state.playoffs.series_data.is_none());

        state.handle_event(AppEvent::SeriesUpdate {
            year: 1930,
            parsed_series: series("A"),
        });
        assert!(state.playoffs.series_data.is_some());
    }

    #[test]
    fn bracket_series_update_requires_selected_season() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.date_state.year = 1931;
        state.handle_event(AppEvent::BracketSeriesUpdate {
            season: "19291930".to_string(),
            letter: "A".to_string(),
            parsed_series: series("A"),
        });
        assert!(state.playoffs.bracket_series_data.is_empty());

        state.handle_event(AppEvent::BracketSeriesUpdate {
            season: "19301931".to_string(),
            letter: "A".to_string(),
            parsed_series: series("A"),
        });
        assert!(state.playoffs.bracket_series_data.contains_key("A"));
    }

    #[test]
    fn late_standings_for_another_date_are_ignored() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        let standings = || {
            crate::models::standings::StandingsResponse::from_json(r#"{"standings":[]}"#).unwrap()
        };
        state.handle_event(AppEvent::StandingsUpdate {
            date: "1999-01-01".to_string(),
            standings: standings(),
            season: None,
        });
        state.handle_event(AppEvent::StandingsOutOfRange {
            date: "1999-01-01".to_string(),
            message: "stale".to_string(),
        });
        assert!(state.standings.standings_data.is_none());
        assert!(state.standings.out_of_range.is_none());

        state.handle_event(AppEvent::StandingsUpdate {
            date: state.date_state.date.to_string(),
            standings: standings(),
            season: None,
        });
        assert!(state.standings.standings_data.is_some());
    }

    #[test]
    fn late_team_stats_for_another_team_or_year_are_ignored() {
        use crate::models::TeamAbbrev;
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.date_state.year = 2027;
        state.team_stats.team_picker.current_team = TeamAbbrev::MTL;
        let stats = || {
            crate::models::team_stats::TeamStatsResponse::from_json(
                r#"{"skaters":[],"goalies":[]}"#,
            )
            .unwrap()
        };

        // Previously selected team
        state.handle_event(AppEvent::TeamStatsRegularSeasonUpdate {
            team: TeamAbbrev::TOR,
            year: 2027,
            stats: stats(),
        });
        // Previously selected year
        state.handle_event(AppEvent::TeamStatsPlayoffsUpdate {
            team: TeamAbbrev::MTL,
            year: 2026,
            stats: stats(),
        });
        state.handle_event(AppEvent::TeamStatsOutOfRange {
            team: TeamAbbrev::TOR,
            year: 2027,
            message: "stale".to_string(),
        });
        assert!(state.team_stats.regular_season_team_stats_data.is_none());
        assert!(state.team_stats.playoffs_team_stats_data.is_none());
        assert!(state.team_stats.out_of_range.is_none());

        state.handle_event(AppEvent::TeamStatsRegularSeasonUpdate {
            team: TeamAbbrev::MTL,
            year: 2027,
            stats: stats(),
        });
        assert!(state.team_stats.regular_season_team_stats_data.is_some());
    }

    #[test]
    fn late_bracket_for_another_year_is_ignored() {
        let mut state = test_state(tokio::sync::mpsc::channel(8).0);
        state.date_state.year = 1930;
        let bracket =
            || crate::models::playoffs::bracket::BracketResponse::from_json("{}").unwrap();
        state.handle_event(AppEvent::BracketUpdate {
            year: 2027,
            bracket: bracket(),
        });
        assert!(state.playoffs.bracket_data.is_none());

        state.handle_event(AppEvent::BracketUpdate {
            year: 1930,
            bracket: bracket(),
        });
        assert!(state.playoffs.bracket_data.is_some());
    }
}
