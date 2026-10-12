use crate::models::games::{
    game_story::{GameStoryResponse, ShootoutAttemptResult},
    games::{AssistInfo, GoalModifier, GoalStrength, PeriodType},
};
use crate::ui::{
    games::games::get_period_title,
    games::games::{BIG_SCORE_COLOR, render_scroll_frame},
    render::{ACCENT_RED, border_style},
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

// Length of the middle chunk for scoring
pub const MIDDLE_LENGTH: u16 = 50;

pub fn render_scoring(
    maybe_game_story: Option<&GameStoryResponse>,
    frame: &mut Frame,
    area: Rect,
    scroll_offset: usize,
    max_scroll: &mut usize,
    visible_rows: &mut usize,
    missing_text: &str,
) {
    let Some(story) = maybe_game_story else {
        *max_scroll = 0;
        *visible_rows = 0;
        frame.render_widget(Line::from(missing_text).centered(), area);
        return;
    };

    let away_team_abbrev = story.away_team.as_ref().and_then(|team| team.abbrev);
    let home_team_abbrev = story.home_team.as_ref().and_then(|team| team.abbrev);

    let mut away_lines = vec![];
    let mut middle_lines = vec![];
    let mut home_lines = vec![];

    if let Some(summary) = &story.summary {
        // Whether any non-shootout period actually contains goals.
        let has_goals = summary.scoring.iter().any(|period_score| {
            !matches!(period_score.period_descriptor.period_type, PeriodType::SO)
                && !period_score.goals.is_empty()
        });
        // No goals yet
        if !has_goals && summary.shootout.is_empty() {
            // Keep all three columns index-aligned
            away_lines.push(Line::default());
            middle_lines.push(
                Line::from("\"No goals.\" - Juuse Saros")
                    .centered()
                    .style(Style::new().fg(Color::DarkGray)),
            );
            home_lines.push(Line::default());
        } else {
            for period_score in summary.scoring.iter() {
                if matches!(period_score.period_descriptor.period_type, PeriodType::SO) {
                    continue;
                }
                if period_score.goals.is_empty() {
                    continue;
                }
                away_lines.push(Line::default());
                middle_lines.push(
                    Line::from(get_period_title(&period_score.period_descriptor))
                        .centered()
                        .style(border_style()),
                );
                home_lines.push(Line::default());
                for (i, goal) in period_score.goals.iter().enumerate() {
                    let goals_to_date = goal
                        .goals_to_date
                        .map(|n| format!(" ({})", n))
                        .unwrap_or_default();

                    let mut strengths = vec![];
                    match goal.strength {
                        GoalStrength::PP => strengths.push("PPG"),
                        GoalStrength::SH => strengths.push("SHG"),
                        GoalStrength::EV => {}
                        _ => {}
                    }

                    match goal.goal_modifier {
                        GoalModifier::EmptyNet => strengths.push("ENG"),
                        GoalModifier::PenaltyShot => strengths.push("Penalty shot"),
                        _ => {}
                    }
                    // Away team goal
                    if Some(goal.team_abbrev.default) == away_team_abbrev {
                        let mut away_spans = vec![];

                        if !strengths.is_empty() {
                            let label = strengths.join(", ");
                            away_spans.push(Span::styled(
                                format!("{} ", label),
                                Style::new().fg(ACCENT_RED).bold(),
                            ));
                        }

                        away_spans.push(Span::raw(format!(
                            "{} {} {}{}",
                            goal.time_in_period, goal.first_name, goal.last_name, goals_to_date
                        )));

                        away_lines.push(Line::default());
                        middle_lines.push(Line::from(away_spans).left_aligned());
                        home_lines.push(Line::default());

                        // Add a line for assists (or display unassisted)
                        away_lines.push(Line::default());
                        middle_lines.push(get_assists_line(&goal.assists, Alignment::Left));
                        home_lines.push(Line::default());
                    }
                    // Home team goal
                    else if Some(goal.team_abbrev.default) == home_team_abbrev {
                        let mut home_spans = vec![Span::raw(format!(
                            "{} {}{} {}",
                            goal.first_name, goal.last_name, goals_to_date, goal.time_in_period
                        ))];

                        if !strengths.is_empty() {
                            let label = strengths.join(", ");
                            home_spans.push(Span::styled(
                                format!(" {}", label),
                                Style::new().fg(ACCENT_RED).bold(),
                            ));
                        }
                        away_lines.push(Line::default());
                        middle_lines.push(Line::from(home_spans).right_aligned());
                        home_lines.push(Line::default());

                        away_lines.push(Line::default());
                        middle_lines.push(get_assists_line(&goal.assists, Alignment::Right));
                        home_lines.push(Line::default());
                    }
                    // Add spacing between goals in the same period.
                    if period_score.goals.get(i + 1).is_some() {
                        away_lines.push(Line::default());
                        middle_lines.push(Line::default());
                        home_lines.push(Line::default());
                    }
                }
            }
            // Add shootout attempts if there are any
            if !summary.shootout.is_empty() {
                // Add shootout lines
                away_lines.push(Line::default());
                middle_lines.push(Line::from("Shootout").centered().style(border_style()));
                home_lines.push(Line::default());
                for shootout_attempt in &summary.shootout {
                    let (attempt_symbol, attempt_color) =
                        if matches!(shootout_attempt.result, ShootoutAttemptResult::Goal) {
                            ("[✓]", BIG_SCORE_COLOR)
                        } else {
                            ("[✗]", ACCENT_RED)
                        };

                    let attempt_span = Span::styled(attempt_symbol, Style::new().fg(attempt_color));

                    if Some(shootout_attempt.team_abbrev.default) == away_team_abbrev {
                        middle_lines.push(
                            Line::from(vec![
                                Span::raw(format!(
                                    "{} {} ",
                                    shootout_attempt.first_name, shootout_attempt.last_name
                                )),
                                attempt_span,
                            ])
                            .left_aligned(),
                        );
                        away_lines.push(Line::default());
                        home_lines.push(Line::default());
                    } else {
                        away_lines.push(Line::default());
                        home_lines.push(Line::default());
                        middle_lines.push(
                            Line::from(vec![
                                attempt_span,
                                Span::raw(format!(
                                    " {} {}",
                                    shootout_attempt.first_name, shootout_attempt.last_name
                                )),
                            ])
                            .right_aligned(),
                        );
                    }
                }
            }
        }
    }

    // Reserve indicator rows, compute the visible window, and render ▲/▼.
    let view = render_scroll_frame(
        frame,
        area,
        away_lines.len(),
        scroll_offset,
        max_scroll,
        visible_rows,
    );

    let visible_away = away_lines[view.range.clone()].to_vec();
    let visible_middle = middle_lines[view.range.clone()].to_vec();
    let visible_home = home_lines[view.range].to_vec();

    let columns =
        crate::ui::games::games::split_info_left_middle_right(view.content, MIDDLE_LENGTH);
    frame.render_widget(Paragraph::new(visible_away), columns[0]);
    frame.render_widget(Paragraph::new(visible_middle), columns[1]);
    frame.render_widget(Paragraph::new(visible_home), columns[2]);
}

fn get_assists_line(assists: &[AssistInfo], alignment: Alignment) -> Line<'static> {
    if !assists.is_empty() {
        let assists_text = get_assists_text(assists);
        Line::styled(
            format!("[{}]", assists_text),
            Style::new().fg(Color::DarkGray),
        )
        .alignment(alignment)
    } else {
        Line::styled("[Unassisted]".to_string(), Style::new().fg(Color::DarkGray))
            .alignment(alignment)
    }
}

fn get_assists_text(assists: &[AssistInfo]) -> String {
    assists
        .iter()
        .map(|assist| {
            format!(
                "{} ({})",
                assist.name,
                assist.assists_to_date.unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
