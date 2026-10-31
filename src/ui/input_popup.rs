use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

/// A centered popup with a title, instruction line, and text input.
/// Used by both the date picker and the player search.
pub struct InputPopup<'a> {
    pub title: &'a str,
    pub instructions: &'a str,
    pub input_text: &'a str,
    pub border_color: Color,
    pub info: Option<&'a str>,
    pub bottom_text: Option<&'a str>,
}

impl Widget for InputPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let [_, instruct, inp] = Layout::vertical([
            Constraint::Length(1), // top border
            Constraint::Length(1), // instructions
            Constraint::Length(1), // input
        ])
        .areas(area);

        Paragraph::new(format!(" {}", self.instructions)).render(instruct, buf);
        Paragraph::new(format!(" {}", self.input_text)).render(inp, buf);
        if let Some(info) = self.info {
            let info_width = text_width(info) + 2; // +2 for padding
            let input_width = text_width(self.input_text) + 2; // +2 for " " prefix and gap
            if inp.width > info_width + input_width {
                Paragraph::new(format!("{} ", info))
                    .right_aligned()
                    .style(Style::new().fg(Color::DarkGray))
                    .render(inp, buf);
            }
        }

        let mut block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(self.border_color));

        if let Some(bottom) = self.bottom_text {
            block = block.title_bottom(Span::styled(
                bottom.to_string(),
                Style::new().fg(Color::DarkGray),
            ));
        }
        block.render(area, buf);
    }
}

/// Create a centered popup rect with the given height and width percentage.
pub fn create_popup(area: Rect, height: u16, percent_width: u16) -> Rect {
    let [_, popup, _] = Layout::vertical(
        [
            Constraint::Ratio(1, 2),
            Constraint::Length(height),
            Constraint::Ratio(1, 2),
        ]
        .as_ref(),
    )
    .areas(area);

    Layout::horizontal(
        [
            Constraint::Percentage((100 - percent_width) / 2),
            Constraint::Percentage(percent_width),
            Constraint::Percentage((100 - percent_width) / 2),
        ]
        .as_ref(),
    )
    .split(popup)[1]
}

/// Terminal columns taken by `text`, not its byte length, so non-ASCII
/// input doesn't push the cursor too far
fn text_width(text: &str) -> u16 {
    u16::try_from(Span::raw(text).width()).unwrap_or(u16::MAX)
}

/// Standard cursor position for an InputPopup, just after `input_text`.
pub fn popup_cursor_position(popup_rect: Rect, input_text: &str) -> (u16, u16) {
    (
        popup_rect
            .x
            .saturating_add(text_width(input_text))
            .saturating_add(1), // +1 for border
        popup_rect.y + 2, // +2 for border and instruction line
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_counts_columns_not_bytes() {
        let rect = Rect::new(10, 5, 40, 3);
        assert_eq!(popup_cursor_position(rect, "abc"), (14, 7));
        // "é" is two bytes but one column
        assert_eq!(popup_cursor_position(rect, "éé"), (13, 7));
    }
}
