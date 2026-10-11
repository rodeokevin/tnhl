use chrono_tz::Tz;
use ratatui::layout::{Direction, Layout};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::models::{
    games::games::{GameState, PeriodType},
    playoffs::series::SeriesResponse,
};
use crate::ui::{
    games::games::{get_period_title, split_info_left_middle_right},
    games::stats::{AWAY_BAR_COLOR, HOME_BAR_COLOR},
    layout::split_area_vertical,
    render::border_style,
};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Style},
    text::Line,
    widgets::Block,
};

use tui_big_text::{BigText, PixelSize};

use crate::ui::games::games::MIDDLE_LENGTH;

pub fn render_series(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered()
        .title(format!(
            " {} Stanley Cup Playoffs ",
            app.state.date_state.year
        ))
        .border_style(border_style());
    let inner = block.inner(area);

    frame.render_widget(block, area);
    let upper_score_schedule = split_area_vertical(
        inner,
        [
            Constraint::Length(4), // upper info (1 at bottom for spacing)
            Constraint::Length(4), // series score (big text)
            Constraint::Fill(1),   // games schedule
        ],
    );

    if let Some(series) = &app.state.playoffs.series_data {
        // Upper info
        let upper_info_chunks = split_area_vertical(
            upper_score_schedule[0],
            [
                Constraint::Length(1), // Round information
                Constraint::Length(1), // Spacing
                Constraint::Length(1), // Teams
            ],
        );

        let round_info = round_display(&series.round_label, &series.round_abbrev);
        let round_info_line = Line::from(round_info).centered();
        frame.render_widget(round_info_line, upper_info_chunks[0]);
        // Teams
        let teams_chunks = split_info_left_middle_right(upper_info_chunks[2], MIDDLE_LENGTH);
        let bottom_seed = if series.bottom_seed_team.id == -1 {
            Line::from("TBD").style(Style::new().fg(Color::DarkGray))
        } else {
            Line::from(format!(
                "{} {}",
                series.bottom_seed_team.place_name.default, series.bottom_seed_team.name.default
            ))
        };
        frame.render_widget(bottom_seed.right_aligned(), teams_chunks[0]);
        frame.render_widget(Line::from("vs").centered(), teams_chunks[1]);
        let top_seed = if series.top_seed_team.id == -1 {
            Line::from("TBD").style(Style::new().fg(Color::DarkGray))
        } else {
            Line::from(format!(
                "{} {}",
                series.top_seed_team.place_name.default, series.top_seed_team.name.default
            ))
        };
        frame.render_widget(top_seed, teams_chunks[2]);

        render_big_series_score(series, frame, upper_score_schedule[1]);

        render_schedule(
            series,
            frame,
            upper_score_schedule[2],
            app.settings.timezone,
            &app.settings.timezone_abbreviation,
            app.state.playoffs.vertical_scroll_offset,
            &mut app.state.playoffs.vertical_max_scroll,
            &mut app.state.playoffs.visible_rows,
        );
    } else {
        frame.render_widget(Line::from("Loading series...").centered(), inner);
    }
}

fn render_big_series_score(series: &SeriesResponse, frame: &mut Frame, area: Rect) {
    let chunks = split_info_left_middle_right(area, MIDDLE_LENGTH);

    let is_total_goals = series.needed_to_win == 0;
    let (bottom_value, top_value) = if is_total_goals {
        let (top_goals, bottom_goals) = aggregate_series_goals(series);
        (format!("{bottom_goals}G"), format!("{top_goals}G"))
    } else {
        (
            series.bottom_seed_team.series_wins.to_string(),
            series.top_seed_team.series_wins.to_string(),
        )
    };

    let bottom_seed_score = build_big_text(bottom_value, Alignment::Right);
    frame.render_widget(bottom_seed_score, chunks[0]);
    let dash = build_big_text("-".to_string(), Alignment::Center);
    frame.render_widget(dash, chunks[1]);
    let top_seed_score = build_big_text(top_value, Alignment::Left);
    frame.render_widget(top_seed_score, chunks[2]);
}

// Prettify the kebab-case from the response
fn round_display(round_label: &str, round_abbrev: &str) -> String {
    let label = round_label.trim();
    if label.is_empty() {
        return round_abbrev.trim().to_string();
    }
    label
        .split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn aggregate_series_goals(series: &SeriesResponse) -> (u32, u32) {
    let top_abbrev = series.top_seed_team.abbrev;
    let bottom_abbrev = series.bottom_seed_team.abbrev;
    let mut top_goals: u32 = 0;
    let mut bottom_goals: u32 = 0;
    for g in &series.games {
        for team in [&g.home_team, &g.away_team] {
            let score = team.score.unwrap_or(0) as u32;
            if team.abbrev == top_abbrev {
                top_goals += score;
            } else if team.abbrev == bottom_abbrev {
                bottom_goals += score;
            }
        }
    }
    (top_goals, bottom_goals)
}

fn build_big_text(text: String, alignment: Alignment) -> BigText<'static> {
    BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::new().fg(HOME_BAR_COLOR))
        .lines(vec![Line::from(text)])
        .alignment(alignment)
        .build()
}

fn render_schedule(
    series: &SeriesResponse,
    frame: &mut Frame,
    area: Rect,
    timezone: Tz,
    timezone_abbr: &str,
    scroll_offset: usize,
    max_scroll: &mut usize,
    visible_rows: &mut usize,
) {
    *visible_rows = area.height.saturating_sub(3) as usize;

    let mut game_number_lines = vec![];
    let mut away_team_lines = vec![];
    let mut away_score_lines = vec![];
    let mut game_status_lines = vec![];
    let mut home_score_lines = vec![];
    let mut home_team_lines = vec![];

    let mut has_if_necessary = false;

    let last_game_index = series.games.len().saturating_sub(1);
    for (game_index, game) in series.games.iter().enumerate() {
        if game.if_necessary {
            has_if_necessary = true;
        }
        let game_label = format!(
            "{}Game {}:",
            if game.if_necessary { "*" } else { "" },
            game.game_number
        );
        game_number_lines.push(Line::from(game_label));

        let date = game
            .compute_local_time(timezone)
            .format("%b %d")
            .to_string();
        game_number_lines.push(Line::from(date).style(Style::new().fg(Color::DarkGray)));

        let away_score = game
            .away_team
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        let home_score = game
            .home_team
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());

        away_score_lines.push(Line::from(away_score).centered());
        away_score_lines.push(Line::default());

        home_score_lines.push(Line::from(home_score).centered());
        home_score_lines.push(Line::default());

        let status_line = match game.game_state {
            GameState::FUT | GameState::PRE => Line::from(format!(
                "{} {}",
                game.compute_local_time(timezone).format("%-I:%M %p"),
                timezone_abbr
            )),

            GameState::LIVE | GameState::CRIT => match game.period_descriptor.as_ref() {
                None => Line::from("Live"),
                Some(d) => Line::styled(get_period_title(d), Style::new().fg(Color::Green)),
            },

            GameState::OVER | GameState::FINAL | GameState::OFF => {
                // The outcome can lag behind the final state, so fall back to "Final".
                let last_period_type = game
                    .game_outcome
                    .as_ref()
                    .map_or(&PeriodType::Unknown, |o| &o.last_period_type);
                let ot_periods = game.game_outcome.as_ref().and_then(|o| o.ot_periods);
                Line::from(match last_period_type {
                    PeriodType::REG | PeriodType::Unknown => "Final".to_string(),
                    PeriodType::OT => match ot_periods.unwrap_or(0) {
                        n if n > 1 => format!("Final/{}OT", n),
                        _ => "Final/OT".to_string(),
                    },
                    PeriodType::SO => "Final/SO".to_string(),
                })
            }

            GameState::Unknown => Line::default(),
        };

        game_status_lines.push(status_line.centered());
        game_status_lines.push(Line::default());

        let winner = match (game.away_team.score, game.home_team.score) {
            (Some(a), Some(h)) if a > h => Some("away"),
            (Some(a), Some(h)) if h > a => Some("home"),
            _ => None,
        };

        let is_final = matches!(
            game.game_state,
            GameState::OVER | GameState::FINAL | GameState::OFF
        );

        let away_is_top = (series.top_seed_team.id != -1)
            && (game.away_team.id == series.top_seed_team.id as u32);
        let away_base_color = if away_is_top {
            AWAY_BAR_COLOR
        } else {
            HOME_BAR_COLOR
        };

        let is_played = matches!(
            game.game_state,
            GameState::OVER | GameState::FINAL | GameState::OFF
        );

        let away_style = if is_played {
            match (winner, is_final) {
                (Some("away"), true) => Style::new().fg(away_base_color).bold(),

                (_, true) => Style::new().fg(Color::DarkGray),

                _ => Style::new().fg(away_base_color),
            }
        } else {
            Style::default()
        };

        let away_team_line = Line::styled(game.away_team.common_name.default.clone(), away_style);

        away_team_lines.push(away_team_line);
        away_team_lines.push(Line::default());

        let home_is_top = (series.top_seed_team.id != -1)
            && (game.home_team.id == series.top_seed_team.id as u32);

        let home_base_color = if home_is_top {
            AWAY_BAR_COLOR
        } else {
            HOME_BAR_COLOR
        };

        let home_style = if is_played {
            match (winner, is_final) {
                (Some("home"), true) => Style::new().fg(home_base_color).bold(),

                (_, true) => Style::new().fg(Color::DarkGray),

                _ => Style::new().fg(home_base_color),
            }
        } else {
            Style::default()
        };

        let home_team_line = Line::styled(
            format!("   {}", game.home_team.common_name.default),
            home_style,
        );

        home_team_lines.push(home_team_line);
        home_team_lines.push(Line::default());

        // Empty line between games (but not after the last game).
        if game_index != last_game_index {
            game_number_lines.push(Line::default());
            away_score_lines.push(Line::default());
            home_score_lines.push(Line::default());
            game_status_lines.push(Line::default());
            away_team_lines.push(Line::default());
            home_team_lines.push(Line::default());
        }
    }

    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let content_height = vert_chunks[1].height as usize;

    let max_scroll_val = game_number_lines.len().saturating_sub(content_height);
    *max_scroll = max_scroll_val;

    let offset = scroll_offset.min(max_scroll_val);

    let can_scroll_up = offset > 0;
    let can_scroll_down = offset < max_scroll_val;

    let end = (offset + content_height).min(game_number_lines.len());

    let visible_game_number_lines = game_number_lines[offset..end].to_vec();
    let visible_away_team_lines = away_team_lines[offset..end].to_vec();
    let visible_away_score_lines = away_score_lines[offset..end].to_vec();
    let visible_game_status_lines = game_status_lines[offset..end].to_vec();
    let visible_home_score_lines = home_score_lines[offset..end].to_vec();
    let visible_home_team_lines = home_team_lines[offset..end].to_vec();

    frame.render_widget(
        Line::from(if can_scroll_up { "▲" } else { "" }).centered(),
        vert_chunks[0],
    );

    frame.render_widget(
        Line::from(if can_scroll_down { "▼" } else { "" }).centered(),
        vert_chunks[2],
    );

    let constraints = [
        Constraint::Fill(1),
        Constraint::Length(1), // For spacing
        Constraint::Length(9),
        Constraint::Length(15),
        Constraint::Length(3),
        Constraint::Length(16),
        Constraint::Length(3),
        Constraint::Length(18),
        Constraint::Fill(1),
    ];

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(vert_chunks[1]);

    frame.render_widget(Paragraph::new(visible_game_number_lines), columns[2]);
    frame.render_widget(Paragraph::new(visible_away_team_lines), columns[3]);
    frame.render_widget(Paragraph::new(visible_away_score_lines), columns[4]);
    frame.render_widget(Paragraph::new(visible_game_status_lines), columns[5]);
    frame.render_widget(Paragraph::new(visible_home_score_lines), columns[6]);
    frame.render_widget(Paragraph::new(visible_home_team_lines), columns[7]);

    if has_if_necessary {
        frame.render_widget(
            Line::from("* If necessary").style(Style::new().fg(Color::DarkGray)),
            columns[0],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{aggregate_series_goals, round_display};
    use crate::models::playoffs::series::SeriesResponse;

    #[test]
    fn round_display_prettifies_kebab_label() {
        assert_eq!(
            round_display("stanley-cup-final", "SCF"),
            "Stanley Cup Final"
        );
        assert_eq!(round_display("1st-round", "R1"), "1st Round");
        assert_eq!(round_display("semifinals", "SF"), "Semifinals");
        assert_eq!(
            round_display("conference-quarterfinals", "CQF"),
            "Conference Quarterfinals"
        );
    }

    #[test]
    fn round_display_falls_back_to_abbrev_when_label_empty() {
        assert_eq!(round_display("", "SCF"), "SCF");
        assert_eq!(round_display("   ", "R1"), "R1");
    }

    fn series_json(
        top: &str,
        bottom: &str,
        games: &[(u8, &str, Option<u8>, &str, Option<u8>)],
    ) -> String {
        let seed = |abbrev: &str| {
            format!(
                r#"{{"id":1,"name":{{"default":"{a}"}},"abbrev":"{a}","placeName":{{"default":"{a}"}},"record":"0-0","seriesWins":0,"seed":1}}"#,
                a = abbrev
            )
        };
        let team = |abbrev: &str, score: Option<u8>| {
            let score = match score {
                Some(s) => s.to_string(),
                None => "null".to_string(),
            };
            format!(
                r#"{{"id":1,"commonName":{{"default":"{a}"}},"abbrev":"{a}","score":{s}}}"#,
                a = abbrev,
                s = score
            )
        };
        let games_json: Vec<String> = games
            .iter()
            .map(|(num, away, as_, home, hs)| {
                format!(
                    r#"{{"id":1,"gameNumber":{num},"ifNecessary":false,"venue":{{"default":"v"}},"startTimeUTC":"2020-01-01T00:00:00Z","gameState":"OFF","awayTeam":{away_t},"homeTeam":{home_t}}}"#,
                    num = num,
                    away_t = team(away, *as_),
                    home_t = team(home, *hs),
                )
            })
            .collect();
        format!(
            r#"{{"round":1,"roundAbbrev":"QF","roundLabel":"quarterfinals","seriesLetter":"A","neededToWin":0,"length":2,"bottomSeedTeam":{bottom},"topSeedTeam":{top},"games":[{games}]}}"#,
            top = seed(top),
            bottom = seed(bottom),
            games = games_json.join(","),
        )
    }

    #[test]
    fn aggregate_sums_goals_per_seed() {
        // top=TAN bottom=MTL; g1 MTL 3 @ TAN 7, g2 TAN 3 @ MTL 4
        // -> TAN 10, MTL 7 (1918 Stanley Cup Final).
        let json = series_json(
            "TAN",
            "MTL",
            &[
                (1, "MTL", Some(3), "TAN", Some(7)),
                (2, "TAN", Some(3), "MTL", Some(4)),
            ],
        );
        let series = SeriesResponse::from_json(&json).unwrap();
        assert_eq!(aggregate_series_goals(&series), (10, 7));
    }

    #[test]
    fn aggregate_treats_missing_score_as_zero() {
        let json = series_json("TAN", "MTL", &[(1, "MTL", None, "TAN", Some(2))]);
        let series = SeriesResponse::from_json(&json).unwrap();
        assert_eq!(aggregate_series_goals(&series), (2, 0));
    }
}
