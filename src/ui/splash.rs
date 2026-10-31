use crate::banner::{BANNER, BIG_BANNER};
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Startup screen shown while today's game day is resolved
pub fn render(f: &mut Frame) {
    let area = f.area();
    // Fall back to the small logo on narrow or short terminals
    let big_width = BIG_BANNER
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let big_height = BIG_BANNER.lines().count() as u16;
    let logo = if area.width >= big_width && area.height > big_height + 2 {
        BIG_BANNER
    } else {
        BANNER.trim_matches('\n')
    };

    let mut lines: Vec<Line> = logo.lines().map(Line::raw).collect();
    lines.push(Line::raw(""));
    lines.push(Line::raw("Loading…").dim());

    let [centered] = Layout::vertical([Constraint::Length(lines.len() as u16)])
        .flex(Flex::Center)
        .areas(area);
    f.render_widget(Paragraph::new(lines).centered(), centered);
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    fn draw(width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(super::render).unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn big_logo_on_roomy_terminals_small_logo_otherwise() {
        let big = draw(80, 24);
        assert!(big.contains("████████╗"));
        assert!(big.contains("Loading…"));
        let small = draw(30, 10);
        assert!(!small.contains("████"));
        assert!(small.contains("/_/ /_/"));
    }
}
