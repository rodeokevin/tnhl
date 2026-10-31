use ratatui::widgets::TableState;

use crate::models::standings::{Grouping, SeasonBounds, StandingsResponse};
use crate::state::app_state::{table_page_down, table_page_up};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandingsFocus {
    WildCard,
    Division,
    Conference,
    League,
}

pub struct StandingsState {
    pub standings_data: Option<StandingsResponse>,
    /// Bounds of the season currently displayed
    pub season: Option<SeasonBounds>,
    /// Set when the requested date is outside the available season range
    pub out_of_range: Option<String>,
    pub table_state: TableState,
    /// Number of visible rows in the table, updated during render
    pub visible_rows: usize,

    pub selected_standings: StandingsFocus,
    pub selected_division: usize,
    pub selected_conference: usize,

    pub divisions: Vec<Grouping>,
    pub conferences: Vec<Grouping>,
    pub has_wildcard: bool,
}

impl Default for StandingsState {
    fn default() -> Self {
        fn table() -> TableState {
            let mut t = TableState::default();
            t.select(Some(0));
            t
        }

        Self {
            standings_data: None,
            season: None,
            out_of_range: None,
            table_state: table(),
            visible_rows: 0,

            selected_standings: StandingsFocus::League,
            selected_division: 0,
            selected_conference: 0,

            divisions: Vec::new(),
            conferences: Vec::new(),
            has_wildcard: false,
        }
    }
}

impl StandingsState {
    pub fn set_data(&mut self, data: StandingsResponse) {
        self.divisions = data.divisions();
        self.conferences = data.conferences();
        self.has_wildcard = data.has_wildcard();
        self.standings_data = Some(data);

        if self.selected_division >= self.divisions.len() {
            self.selected_division = 0;
        }
        if self.selected_conference >= self.conferences.len() {
            self.selected_conference = 0;
        }
        // If the current tab isn't available this season, fall back to League.
        if !self.available_tabs().contains(&self.selected_standings) {
            self.selected_standings = StandingsFocus::League;
        }
        self.clamp_selected_row();
    }

    /// Keep the selected row within the current table's bounds without snapping
    /// to the top, so periodic refreshes don't disturb the user's selection.
    fn clamp_selected_row(&mut self) {
        let len = self.current_table_len();
        let max = len.saturating_sub(1);
        match self.table_state.selected() {
            Some(sel) if sel > max => self.table_state.select(Some(max)),
            Some(_) => {}
            None => self.table_state.select(Some(0)),
        }
        self.skip_label_row(true);
    }

    /// The standings tabs available for the current season, in display order.
    /// League is always present; the others depend on the data.
    pub fn available_tabs(&self) -> Vec<StandingsFocus> {
        let mut tabs = Vec::new();
        if self.has_wildcard && !self.conferences.is_empty() {
            tabs.push(StandingsFocus::WildCard);
        }
        if !self.divisions.is_empty() {
            tabs.push(StandingsFocus::Division);
        }
        if !self.conferences.is_empty() {
            tabs.push(StandingsFocus::Conference);
        }
        tabs.push(StandingsFocus::League);
        tabs
    }

    /// The currently selected division grouping, if any.
    pub fn current_division(&self) -> Option<&Grouping> {
        self.divisions.get(self.selected_division)
    }
    /// The currently selected conference grouping, if any.
    pub fn current_conference(&self) -> Option<&Grouping> {
        self.conferences.get(self.selected_conference)
    }

    pub fn current_table_len(&self) -> usize {
        match self.selected_standings {
            StandingsFocus::League => self
                .standings_data
                .as_ref()
                .map_or(0, |d| d.standings.len()),
            StandingsFocus::Conference => self.count_in_conference(),
            StandingsFocus::Division => self.count_in_division(),
            // Conference + 3 label rows.
            StandingsFocus::WildCard => self.count_in_conference() + 3,
        }
    }

    fn count_in_division(&self) -> usize {
        let (Some(div), Some(data)) = (self.current_division(), &self.standings_data) else {
            return 0;
        };
        data.standings
            .iter()
            .filter(|t| t.division_abbrev.as_deref() == Some(div.abbrev.as_str()))
            .count()
    }

    fn count_in_conference(&self) -> usize {
        let (Some(conf), Some(data)) = (self.current_conference(), &self.standings_data) else {
            return 0;
        };
        data.standings
            .iter()
            .filter(|t| t.conference_abbrev.as_deref() == Some(conf.abbrev.as_str()))
            .count()
    }

    /// Rows of the Wild Card table that are division or "Wildcard" labels,
    /// laid out the same way as `render_wildcard`
    fn wildcard_label_rows(&self) -> Vec<usize> {
        let (Some(conf), Some(data)) = (self.current_conference(), &self.standings_data) else {
            return Vec::new();
        };
        let mut labels = Vec::new();
        let mut row = 0;
        for div in data.divisions_in_conference(&conf.abbrev) {
            labels.push(row);
            let teams = data
                .standings
                .iter()
                .filter(|t| t.division_abbrev.as_deref() == Some(div.abbrev.as_str()))
                .count();
            row += 1 + teams.min(3);
        }
        labels.push(row);
        labels
    }

    /// On the Wild Card tab, move the selection off a label row, keeping the
    /// direction of travel unless that would leave the table
    fn skip_label_row(&mut self, down: bool) {
        if self.selected_standings != StandingsFocus::WildCard {
            return;
        }
        let Some(selected) = self.table_state.selected() else {
            return;
        };
        if !self.wildcard_label_rows().contains(&selected) {
            return;
        }
        let row = if (down && selected + 1 < self.current_table_len()) || selected == 0 {
            selected + 1
        } else {
            selected - 1
        };
        self.table_state.select(Some(row));
    }

    /// Select a new row in the standings table
    pub fn row_down(&mut self) {
        self.table_state.scroll_down_by(1);
        self.skip_label_row(true);
    }
    pub fn row_up(&mut self) {
        self.table_state.scroll_up_by(1);
        self.skip_label_row(false);
    }
    pub fn page_up(&mut self) {
        table_page_up(self.visible_rows, &mut self.table_state);
        self.skip_label_row(false);
    }
    pub fn page_down(&mut self) {
        table_page_down(
            self.visible_rows,
            self.current_table_len(),
            &mut self.table_state,
        );
        self.skip_label_row(true);
    }

    pub fn shift_standings_type(&mut self, next: bool) -> bool {
        let tabs = self.available_tabs();
        let Some(pos) = tabs.iter().position(|t| *t == self.selected_standings) else {
            self.selected_standings = StandingsFocus::League;
            return true;
        };
        let new_pos = if next {
            (pos + 1).min(tabs.len() - 1)
        } else {
            pos.saturating_sub(1)
        };
        if new_pos == pos {
            return false;
        }
        self.selected_standings = tabs[new_pos];
        true
    }

    pub fn has_cyclable_subtype(&self) -> bool {
        match self.selected_standings {
            StandingsFocus::Conference | StandingsFocus::WildCard => self.conferences.len() > 1,
            StandingsFocus::Division => self.divisions.len() > 1,
            StandingsFocus::League => false,
        }
    }

    pub fn cycle_display(&mut self, next: bool) -> bool {
        if !self.has_cyclable_subtype() {
            return false;
        }
        match self.selected_standings {
            StandingsFocus::Conference | StandingsFocus::WildCard => {
                self.selected_conference =
                    cycle_index(self.selected_conference, self.conferences.len(), next);
            }
            StandingsFocus::Division => {
                self.selected_division =
                    cycle_index(self.selected_division, self.divisions.len(), next);
            }
            StandingsFocus::League => {}
        }
        true
    }

    /// Reset standings to default state
    pub fn reset_state(&mut self) {
        self.reset_table_state();
        self.selected_standings = StandingsFocus::League;
        self.selected_conference = 0;
        self.selected_division = 0;
    }
    /// Reset selected row in table
    pub fn reset_table_state(&mut self) {
        self.table_state.select(Some(0));
        self.skip_label_row(true);
    }
}

/// Wrap an index forward/backward within `len` (no-op if `len` is 0).
fn cycle_index(current: usize, len: usize, next: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if next {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    }
}
