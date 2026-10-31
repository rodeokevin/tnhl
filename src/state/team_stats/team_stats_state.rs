use crate::models::team_stats::TeamStatsResponse;
use crate::state::{
    app_state::{table_page_down, table_page_up},
    team_stats::team_picker::TeamPickerState,
};
use ratatui::widgets::TableState;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlayerType {
    #[default]
    Skaters,
    Goalies,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GameType {
    #[default]
    RegularSeason,
    Playoffs,
}

impl GameType {
    pub fn toggle(&self) -> Self {
        match self {
            GameType::Playoffs => GameType::RegularSeason,
            GameType::RegularSeason => GameType::Playoffs,
        }
    }
}

pub struct TeamStatsState {
    pub regular_season_team_stats_data: Option<TeamStatsResponse>,
    pub playoffs_team_stats_data: Option<TeamStatsResponse>,
    pub out_of_range: Option<String>,
    /// Shown instead of "Loading" when the regular season stats couldn't be loaded
    pub regular_season_unavailable: Option<String>,
    /// Shown instead of "Loading" when the playoffs stats couldn't be loaded
    pub playoffs_unavailable: Option<String>,
    pub table_state: TableState,
    /// Number of visible rows in the table, updated during render
    pub visible_rows: usize,
    pub player_type: PlayerType,
    pub game_type: GameType,

    pub team_picker: TeamPickerState,
}

impl Default for TeamStatsState {
    fn default() -> Self {
        fn table() -> TableState {
            let mut t = TableState::default();
            t.select(Some(0));
            t
        }

        Self {
            regular_season_team_stats_data: None,
            playoffs_team_stats_data: None,
            out_of_range: None,
            regular_season_unavailable: None,
            playoffs_unavailable: None,
            table_state: table(),
            visible_rows: 0,
            player_type: PlayerType::default(),
            game_type: GameType::default(),
            team_picker: TeamPickerState::default(),
        }
    }
}

impl TeamStatsState {
    /// Return the length of the table
    pub fn current_table_len(&self) -> usize {
        match self.game_type {
            GameType::RegularSeason => self
                .regular_season_team_stats_data
                .as_ref()
                .map(|data| match self.player_type {
                    PlayerType::Skaters => data.skaters.len(),
                    PlayerType::Goalies => data.goalies.len(),
                })
                .unwrap_or(0),
            GameType::Playoffs => self
                .playoffs_team_stats_data
                .as_ref()
                .map(|data| match self.player_type {
                    PlayerType::Skaters => data.skaters.len(),
                    PlayerType::Goalies => data.goalies.len(),
                })
                .unwrap_or(0),
        }
    }
    /// Move rows
    pub fn row_up(&mut self) {
        self.table_state.scroll_up_by(1);
    }
    pub fn row_down(&mut self) {
        self.table_state.scroll_down_by(1);
    }
    pub fn page_up(&mut self) {
        table_page_up(self.visible_rows, &mut self.table_state);
    }
    pub fn page_down(&mut self) {
        table_page_down(
            self.visible_rows,
            self.current_table_len(),
            &mut self.table_state,
        );
    }
    /// Reset team stats to default state
    pub fn reset_state(&mut self) {
        self.reset_table_state();
        self.game_type = GameType::default();
        self.player_type = PlayerType::default();
    }
    /// Reset selected row in table
    pub fn reset_table_state(&mut self) {
        self.table_state.select(Some(0));
    }
}
