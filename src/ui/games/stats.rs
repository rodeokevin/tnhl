use crate::App;
use crate::models::games::game_story::{GameStatsCategory, StatValue, TeamGameStats};
use crate::ui::{
    games::games::{render_scroll_frame, split_info_left_middle_right},
    render::{ACCENT_BLUE, ACCENT_RED, border_style},
};
use std::collections::HashMap;
use std::vec;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

// Length of the middle chunk for scoring
pub const STATS_MIDDLE_LENGTH: u16 = 25;

pub fn render_stats(frame: &mut Frame, app: &mut App, area: Rect) {
    let game_id = app
        .state
        .games
        .games_data
        .as_ref()
        .and_then(|g| g.games.get(app.state.games.selected_game_index))
        .map(|g| g.id);
    let game_story = game_id.and_then(|id| app.state.games.game_story_data.get(&id));

    let Some(game_story) = game_story else {
        app.state.games.max_scroll = 0;
        app.state.games.visible_rows = 0;
        let missing = app.state.games.missing_text(
            &app.state.games.game_story_unavailable,
            "Loading stats...",
            "No game details available for this game.",
        );
        frame.render_widget(Line::from(missing).centered(), area);
        return;
    };

    let mut away_lines = vec![];
    let mut middle_lines = vec![];
    let mut home_lines = vec![];

    if let Some(summary) = &game_story.summary
        && !summary.team_game_stats.is_empty()
    {
        let stats_map: HashMap<_, _> = summary
            .team_game_stats
            .iter()
            .map(|stat| (stat.category.clone(), stat))
            .collect();

        // Shots on goal
        if let Some(&sog) = stats_map.get(&GameStatsCategory::Sog) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Shots on Goal".to_string(),
                sog,
                sog.away_value.to_string(),
                sog.home_value.to_string(),
            );
        }
        // Face-off %
        if let Some(&faceoff) = stats_map.get(&GameStatsCategory::FaceoffWinningPctg) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Face-off %".to_string(),
                faceoff,
                format!("{}%", faceoff.away_value),
                format!("{}%", faceoff.home_value),
            );
        }
        // Power Play %
        if let Some(&power_play_pctg) = stats_map.get(&GameStatsCategory::PowerPlayPctg) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Power Play %".to_string(),
                power_play_pctg,
                format!("{}%", power_play_pctg.away_value),
                format!("{}%", power_play_pctg.home_value),
            );
            if let Some(&power_play_rate) = stats_map.get(&GameStatsCategory::PowerPlay) {
                away_lines.push(
                    Line::from(power_play_rate.away_value.to_string())
                        .style(Style::new().fg(Color::DarkGray))
                        .right_aligned(),
                );
                middle_lines.push(Line::default().centered());
                home_lines.push(
                    Line::from(power_play_rate.home_value.to_string())
                        .style(Style::new().fg(Color::DarkGray))
                        .left_aligned(),
                );
            }
        }
        // Penalty minutes
        if let Some(&pims) = stats_map.get(&GameStatsCategory::Pim) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Penalty Minutes".to_string(),
                pims,
                pims.away_value.to_string(),
                pims.home_value.to_string(),
            );
        }
        // Hits
        if let Some(&hits) = stats_map.get(&GameStatsCategory::Hits) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Hits".to_string(),
                hits,
                hits.away_value.to_string(),
                hits.home_value.to_string(),
            );
        }
        // Blocked Shots
        if let Some(&blocked_shots) = stats_map.get(&GameStatsCategory::BlockedShots) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Blocked Shots".to_string(),
                blocked_shots,
                blocked_shots.away_value.to_string(),
                blocked_shots.home_value.to_string(),
            );
        }
        // Giveaways
        if let Some(&giveaways) = stats_map.get(&GameStatsCategory::Giveaways) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Giveaways".to_string(),
                giveaways,
                giveaways.away_value.to_string(),
                giveaways.home_value.to_string(),
            );
        }
        // Takeaways
        if let Some(&takeaways) = stats_map.get(&GameStatsCategory::Takeaways) {
            add_stat_lines(
                &mut away_lines,
                &mut middle_lines,
                &mut home_lines,
                "Takeaways".to_string(),
                takeaways,
                takeaways.away_value.to_string(),
                takeaways.home_value.to_string(),
            );
        }
    }

    // Reserve indicator rows, compute the visible window, and render ▲/▼.
    let view = render_scroll_frame(
        frame,
        area,
        away_lines.len(),
        app.state.games.scroll_offset,
        &mut app.state.games.max_scroll,
        &mut app.state.games.visible_rows,
    );

    let visible_away: Vec<Line> = away_lines[view.range.clone()].to_vec();
    let visible_home: Vec<Line> = home_lines[view.range.clone()].to_vec();
    let visible_period: Vec<Line> = middle_lines[view.range].to_vec();

    // Re-split the content area horizontally
    let chunks = split_info_left_middle_right(view.content, 25);

    frame.render_widget(Paragraph::new(visible_away), chunks[0]);
    frame.render_widget(Paragraph::new(visible_period), chunks[1]);
    frame.render_widget(Paragraph::new(visible_home), chunks[2]);
}

// Add the lines for one stat
fn add_stat_lines(
    away_lines: &mut Vec<Line<'_>>,
    middle_lines: &mut Vec<Line<'_>>,
    home_lines: &mut Vec<Line<'_>>,
    title: String,
    stat: &TeamGameStats,
    away_value: String,
    home_value: String,
) {
    away_lines.push(Line::default());
    middle_lines.push(Line::from(title).style(border_style()).centered());
    home_lines.push(Line::default());

    away_lines.push(Line::from(away_value).right_aligned());
    middle_lines.push(compute_middle_bar(&stat.away_value, &stat.home_value));
    home_lines.push(Line::from(home_value).left_aligned());
}

fn compute_middle_bar<'a>(away_value: &'a StatValue, home_value: &'a StatValue) -> Line<'static> {
    let mut away_length = 0;
    let mut home_length = 0;
    let (away_zero, home_zero) = (away_value.is_zero(), home_value.is_zero());
    if away_zero && home_zero {
        away_length = (STATS_MIDDLE_LENGTH - 3) / 2;
        home_length = (STATS_MIDDLE_LENGTH - 3) / 2;
    } else if away_zero {
        home_length = STATS_MIDDLE_LENGTH - 2;
    } else if home_zero {
        away_length = STATS_MIDDLE_LENGTH - 2;
    } else {
        let total = (STATS_MIDDLE_LENGTH - 3) as f64;
        let away = match away_value {
            StatValue::Int(v) => *v as f64,
            StatValue::Float(v) => *v,
            _ => 0.0,
        };
        let home = match home_value {
            StatValue::Int(v) => *v as f64,
            StatValue::Float(v) => *v,
            _ => 0.0,
        };
        let sum = away + home;
        // Clamped in case a negative value pushes the share outside 0..=total
        away_length = ((away / sum) * total).round().clamp(1.0, total) as u16;
        home_length = (STATS_MIDDLE_LENGTH - 3).saturating_sub(away_length);
    }
    let gap = std::iter::once(Span::raw(if away_zero ^ home_zero { "" } else { " " }));
    let away_spans: Vec<_> = std::iter::repeat_n(
        Span::styled("─", Style::new().fg(ACCENT_RED)),
        away_length as usize,
    )
    .collect();
    let home_spans: Vec<_> = std::iter::repeat_n(
        Span::styled("─", Style::new().fg(ACCENT_BLUE)),
        home_length as usize,
    )
    .collect();
    let spans: Vec<_> = away_spans
        .into_iter()
        .chain(gap)
        .chain(home_spans)
        .collect();

    Line::from(spans).centered()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar_width(line: &Line) -> usize {
        line.spans.iter().map(|s| s.content.chars().count()).sum()
    }

    #[test]
    fn middle_bar_stays_in_bounds_with_negative_values() {
        // A negative value used to push the away share past the bar width
        let line = compute_middle_bar(&StatValue::Float(5.0), &StatValue::Float(-3.0));
        assert!(bar_width(&line) <= STATS_MIDDLE_LENGTH as usize);
    }
}
