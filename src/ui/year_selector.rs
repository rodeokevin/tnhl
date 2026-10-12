use crate::state::date_state::DateState;
use crate::ui::{
    input_popup::InputPopup,
    render::{ACCENT_RED, BORDER_COLOR},
};
use ratatui::prelude::*;

pub struct YearSelectorWidget {}

impl StatefulWidget for YearSelectorWidget {
    type State = DateState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_color = if state.is_valid {
            BORDER_COLOR
        } else {
            ACCENT_RED
        };
        InputPopup {
            title: "Enter a year (YYYY) or use arrow keys",
            instructions: "Press Enter to submit or Esc to cancel",
            input_text: &state.text,
            border_color,
            info: None,
            bottom_text: Some("Hint: enter t for current year"),
        }
        .render(area, buf);
    }
}
