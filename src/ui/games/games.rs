use crate::app::App;
use crate::models::TeamAbbrev;
use crate::models::games::games::{
    GameData, GameState, PeriodDescriptor, PeriodType, SeriesStatus, SituationDesc,
};
use crate::models::playoffs::series::SeriesResponse;
use crate::state::games_state::GamesFocus;
use crate::ui::games::stats::AWAY_BAR_COLOR;
use crate::ui::{
    games::{
        boxscore,
        play_by_play::{play_by_play, rink},
        pregame, scoring, stats,
    },
    layout::{split_area_horizontal, split_area_vertical, tabs_and_content},
    render::{BORDER_COLOR, border_style},
};
use chrono_tz::Tz;
use std::rc::Rc;
use std::vec;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Tabs},
};

use tui_big_text::{BigText, PixelSize};

pub const MIDDLE_LENGTH: u16 = 10;
pub const BIG_SCORE_COLOR: Color = Color::Rgb(35, 179, 16); // Green

/// Minimum inner height required before the rink is shown, so small terminals
/// don't get a cramped layout.
const MIN_HEIGHT_FOR_RINK: u16 = 24;

pub fn render_games(frame: &mut Frame, app: &mut App, area: Rect) {
    // Split content chunk into tab + content
    let tab_content_chunks = tabs_and_content(area);

    let game_dates = app.state.games.games_data.as_ref().and_then(|d| {
        let prev = d.prev_date.as_deref();
        let next = d.next_date.as_deref();
        if prev.is_none() && next.is_none() {
            return None;
        }
        Some(
            Line::from(format!(
                " prev: {}  next: {} ",
                prev.unwrap_or("-"),
                next.unwrap_or("-")
            ))
            .style(Style::new().fg(Color::DarkGray))
            .right_aligned(),
        )
    });

    let favorite = app.settings.favorite_team;
    let matchups: Vec<Line> = app
        .state
        .games
        .games_data
        .as_ref()
        .map(|data| {
            data.games
                .iter()
                .map(|game| {
                    // Favorite team playing in this matchup overrides the
                    // game-state color with gold
                    let is_favorite = favorite.is_some_and(|fav| {
                        fav == game.away_team.abbrev || fav == game.home_team.abbrev
                    });
                    let color = if is_favorite {
                        Style::new().fg(BORDER_COLOR)
                    } else {
                        get_color_from_game_state(&game.game_state)
                    };
                    Line::from(format!(
                        "{} @ {}",
                        game.away_team.abbrev, game.home_team.abbrev
                    ))
                    .style(color.add_modifier(Modifier::BOLD))
                })
                .collect()
        })
        .unwrap_or_default();
    let num_matchups = matchups.len();

    let selected_color = Style::new().underlined();

    // Compute the displayed tabs
    let available_width = (tab_content_chunks[0].width - 2) as usize;
    let selected = app.state.games.selected_game_index;

    const ARROW_WIDTH: usize = 3;
    const MATCHUP_TAB_WIDTH: usize = 12;

    let mut has_left = false;
    let mut has_right = false;
    let (start, end) = loop {
        let arrow_width_total =
            (if has_left { ARROW_WIDTH } else { 0 }) + (if has_right { ARROW_WIDTH } else { 0 });

        let usable_width = available_width.saturating_sub(arrow_width_total);
        let max_tabs = (usable_width / MATCHUP_TAB_WIDTH).max(1);

        let page = selected / max_tabs;
        let start = page * max_tabs;
        let end = (start + max_tabs).min(num_matchups);

        let new_has_left = start > 0;
        let new_has_right = end < num_matchups;

        if new_has_left == has_left && new_has_right == has_right {
            break (start, end);
        }

        has_left = new_has_left;
        has_right = new_has_right;
    };

    let mut visible_matchups: Vec<Line> = matchups[start..end].to_vec();
    if has_right {
        visible_matchups.push(Line::from(">"));
    }
    if has_left {
        visible_matchups.insert(0, Line::from("<"));
    }

    let date_title = app.state.date_state.format_date_border_title();
    let make_block = || {
        let mut block = Block::bordered()
            .border_style(border_style())
            .title(date_title.clone());
        if let Some(dates) = game_dates.clone() {
            block = block.title_top(dates);
        }
        block
    };

    if app.state.games.games_data.is_none() {
        let tabs = Tabs::new(vec!["Loading games..."])
            .block(make_block())
            .highlight_style(Style::default());

        frame.render_widget(tabs, tab_content_chunks[0]);
    } else if num_matchups == 0 {
        let tabs = Tabs::new(vec!["No games today :("])
            .block(make_block())
            .highlight_style(Style::default());

        frame.render_widget(tabs, tab_content_chunks[0]);
    } else {
        let local_selected = selected - start;
        let offset = if has_left { 1 } else { 0 };

        let tabs = Tabs::new(visible_matchups)
            .select(local_selected + offset)
            .block(make_block())
            .highlight_style(selected_color);

        frame.render_widget(tabs, tab_content_chunks[0]);
    }

    // Keep the focus in sync with the selected game's state (pre-game vs. live)
    app.state.games.sync_focus_to_game_state();

    let block = Block::bordered().border_style(border_style());
    let inner = block.inner(tab_content_chunks[1]);
    frame.render_widget(block, tab_content_chunks[1]);

    let show_rink = app.state.games.plays_visible && inner.height >= MIN_HEIGHT_FOR_RINK;

    // Coordinates of the selected play
    let play_coords = if show_rink {
        app.state.games.selected_play_coords()
    } else {
        None
    };

    // Whether the selected game is a playoff game to show series info
    let is_playoff = app.state.games.is_playoff();

    // Split the page into upper and lower regions
    const UPPER_INFO_HEIGHT: u16 = 5;
    const SCORE_HEIGHT: u16 = 4;
    const SERIES_INFO_HEIGHT: u16 = 2; // playoff: series label + status
    const MIN_LOWER_HEIGHT: u16 = 8;
    let series_height = if is_playoff { SERIES_INFO_HEIGHT } else { 0 };

    let left_height = UPPER_INFO_HEIGHT + SCORE_HEIGHT + series_height;

    let upper_region_height = if show_rink {
        // Right half width drives the aspect-correct rink height.
        let rink_width = inner.width / 2;
        let rink_height = rink::rows_for_width(rink_width);

        let max_region = inner.height.saturating_sub(MIN_LOWER_HEIGHT);
        rink_height.max(left_height).min(max_region)
    } else {
        left_height
    };

    let region_chunks = split_area_vertical(
        inner,
        [
            Constraint::Length(upper_region_height), // info + score + series + rink if toggled
            Constraint::Fill(1),                     // lower info
        ],
    );

    // Left half carries the info/score column; right half carries the rink.
    let (upper_region, rink_area) = if show_rink {
        let halves = split_area_horizontal(
            region_chunks[0],
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        );
        (halves[0], Some(halves[1]))
    } else {
        (region_chunks[0], None)
    };

    // Sub-split the (left) upper region into: info band, score, and series info
    let upper_score = split_area_vertical(
        upper_region,
        [
            Constraint::Length(UPPER_INFO_HEIGHT), // upper info (1 for spacing)
            Constraint::Length(SCORE_HEIGHT),      // score (big text)
            Constraint::Length(series_height),     // playoff series info (0 if not playoffs)
            Constraint::Min(0),
        ],
    );
    let series_area = upper_score[2];

    let upper_score_lower = [upper_score[0], upper_score[1], region_chunks[1]];

    if let Some(rink_area) = rink_area {
        rink::render_rink(frame, rink_area, play_coords);
    }

    // Render game information
    let total_goals_status: Option<String> = app.state.games.selected_game().and_then(|g| {
        let viewed_game_number = g.series_status.as_ref()?.game_number_of_series;
        let series = app.state.games.total_goals_data.get(&g.id)?;
        Some(total_goals_status_line(series, viewed_game_number))
    });
    if let Some(games_data) = &mut app.state.games.games_data {
        if let Some(game) = games_data.games.get(app.state.games.selected_game_index) {
            // Upper info
            let upper_info_chunks = split_area_vertical(
                upper_score_lower[0],
                [
                    Constraint::Length(1), // Time remaining
                    Constraint::Length(1), // Status bar
                    Constraint::Length(1), // Teams and strength status
                    Constraint::Length(1), // Shots on goal
                ],
            );
            render_time_remaining(
                game,
                app.settings.timezone,
                &app.settings.timezone_abbreviation,
                frame,
                upper_info_chunks[0],
            );
            render_sweeping_status(
                game,
                10,
                app.state.games.sweeping_status_offset,
                frame,
                upper_info_chunks[1],
            );
            render_team_status(game, favorite, frame, upper_info_chunks[2]);
            // Show record if pregame, sog if live or ended
            if app.state.games.focus == GamesFocus::Pregame {
                render_season_records(
                    app.state.games.game_story_data.get(&game.id),
                    frame,
                    upper_info_chunks[3],
                );
            } else {
                render_shots_on_goal(game, frame, upper_info_chunks[3]);
            }
            render_big_score(game, frame, upper_score_lower[1]);

            if let Some(series) = &game.series_status {
                let series_chunks = split_area_vertical(
                    series_area,
                    [
                        Constraint::Length(1), // Playoff information
                        Constraint::Length(1), // Playoff status
                    ],
                );
                render_series_info(series, frame, series_chunks[0]);
                render_series_status(
                    series,
                    total_goals_status.as_deref(),
                    frame,
                    series_chunks[1],
                );
            }

            // The lower region is the main content
            let main_area = upper_score_lower[2];

            let show_plays =
                app.state.games.plays_visible && app.state.games.focus != GamesFocus::Pregame;
            let (main_area, plays_area) = if show_plays {
                let halves = split_area_horizontal(
                    main_area,
                    [Constraint::Percentage(50), Constraint::Percentage(50)],
                );
                (halves[0], Some(halves[1]))
            } else {
                (main_area, None)
            };

            let main_focused = !app.state.games.plays_focused;
            let main_border = if main_focused {
                border_style()
            } else {
                Style::new().fg(Color::DarkGray)
            };
            let main_block = Block::bordered()
                .title(get_block_title(&app.state.games.focus))
                .border_style(main_border);
            let main_inner = main_block.inner(main_area);
            frame.render_widget(main_block, main_area);

            match &app.state.games.focus {
                // Before a game goes live, show the pre-game matchup
                GamesFocus::Pregame => {
                    pregame::render_pregame(
                        app.state.games.game_story_data.get(&game.id),
                        frame,
                        main_inner,
                        app.state.games.scroll_offset,
                        &mut app.state.games.max_scroll,
                        &mut app.state.games.visible_rows,
                    );
                }
                GamesFocus::Scoring => {
                    scoring::render_scoring(
                        app.state.games.game_story_data.get(&game.id),
                        frame,
                        main_inner,
                        app.state.games.scroll_offset,
                        &mut app.state.games.max_scroll,
                        &mut app.state.games.visible_rows,
                    );
                }
                GamesFocus::Boxscore => {
                    boxscore::render_boxscore(frame, app, main_inner);
                }
                GamesFocus::Stats => {
                    stats::render_stats(frame, app, main_inner);
                }
            }

            if let Some(plays_area) = plays_area {
                play_by_play::render_play_by_play(frame, app, plays_area);
            }
        }
    }
}

pub fn get_color_from_game_state(state: &GameState) -> Style {
    match state {
        GameState::LIVE | GameState::CRIT | GameState::OVER => Style::new().fg(Color::Green),
        GameState::FINAL | GameState::OFF => Style::new().fg(Color::DarkGray),
        _ => Style::new().fg(Color::White), // FUT, PRE, Unknown
    }
}

pub fn render_time_remaining(
    game: &GameData,
    timezone: Tz,
    timezone_abbr: &str,
    frame: &mut Frame,
    area: Rect,
) {
    // Not in intermission
    if matches!(game.game_state, GameState::LIVE | GameState::CRIT) {
        if let (Some(clock), Some(period)) = (&game.clock, &game.period_descriptor) {
            if !clock.in_intermission {
                let chunks = split_area_horizontal(
                    area,
                    [
                        Constraint::Fill(1),
                        Constraint::Length(22),
                        Constraint::Fill(1),
                    ],
                );

                let time = Line::from(format!(
                    "{} - {}",
                    get_period_title(period),
                    clock.time_remaining,
                ))
                .centered();

                frame.render_widget(time, chunks[1]);

                let spans: Vec<Span> = game
                    .situation
                    .as_ref()
                    .map(|s| {
                        vec![Span::styled(
                            format!(
                                " {}-on-{}",
                                s.away_team.strength.max(s.home_team.strength),
                                s.away_team.strength.min(s.home_team.strength)
                            ),
                            Style::new().fg(AWAY_BAR_COLOR).bold(),
                        )]
                    })
                    .unwrap_or_default();
                frame.render_widget(Line::from(spans).left_aligned(), chunks[2]);
                return;
            }
        }
    }
    let line = match game.game_state {
        GameState::FUT | GameState::PRE => Line::from(format!(
            "{} {}",
            game.compute_local_time(timezone).format("%-I:%M %p"),
            timezone_abbr,
        )),
        GameState::LIVE | GameState::CRIT => {
            // in intermission or clock is None
            match (game.clock.as_ref(), game.period_descriptor.as_ref()) {
                (None, _) | (_, None) => Line::from("Live"),
                (Some(clock), Some(period)) => match period.period_type {
                    PeriodType::REG | PeriodType::OT => Line::from(format!(
                        "End of {} ({})",
                        get_period_title(period),
                        clock.time_remaining
                    )),
                    PeriodType::SO => Line::from("End of Shootout"),
                    _ => Line::from("Intermission"),
                },
            }
        }
        GameState::OVER | GameState::FINAL | GameState::OFF => {
            // The outcome can lag behind the final state, so fall back to "Final".
            let Some(outcome) = game.game_outcome.as_ref() else {
                return frame.render_widget(Line::from("Final").centered(), area);
            };
            match outcome.last_period_type {
                PeriodType::REG | PeriodType::Unknown => Line::from("Final"),
                PeriodType::OT => match outcome.ot_periods.unwrap_or(0) {
                    n if n > 1 => Line::from(format!("Final/{}OT", n)),
                    _ => Line::from("Final/OT"),
                },
                PeriodType::SO => Line::from("Final/SO"),
            }
        }
        GameState::Unknown => {
            log::warn!("Unknown game state");
            Line::default()
        }
    };

    frame.render_widget(line.centered(), area);
}

pub fn render_sweeping_status(
    game: &GameData,
    width: usize,
    offset: usize,
    frame: &mut Frame,
    area: Rect,
) {
    match game.game_state {
        GameState::LIVE | GameState::CRIT if game.clock.is_some() => {
            if let Some(clock) = &game.clock {
                let chunks = split_info_left_middle_right(area, MIDDLE_LENGTH);
                if clock.running {
                    let spans: Vec<_> = (0..width)
                        .map(|i| {
                            let pos = offset % width;
                            let dist = i.abs_diff(pos).min(width - i.abs_diff(pos));
                            if dist == 0 || dist == 1 {
                                Span::styled("━", Style::new().fg(Color::Green))
                            } else {
                                Span::styled("─", Style::new().fg(Color::DarkGray))
                            }
                        })
                        .collect();
                    frame.render_widget(Line::from(spans).centered(), chunks[1]);
                } else {
                    let spans: Vec<_> =
                        std::iter::repeat(Span::styled("─", Style::new().fg(Color::Red)))
                            .take(width)
                            .collect();

                    frame.render_widget(Line::from(spans).centered(), chunks[1]);
                }
            }
        }
        _ => frame.render_widget(Line::default(), area),
    }
}

pub fn render_team_status(
    game: &GameData,
    favorite: Option<TeamAbbrev>,
    frame: &mut Frame,
    area: Rect,
) {
    let chunks = split_info_left_middle_right(area, MIDDLE_LENGTH);

    // Style the favorite team's name gold when it's playing in this game.
    let name_style = |abbrev: TeamAbbrev| {
        if favorite == Some(abbrev) {
            Style::new().fg(BORDER_COLOR).bold()
        } else {
            Style::default()
        }
    };

    let mut left_spans = vec![];
    let situation = game.situation.as_ref();

    if let Some(s) = situation {
        if let Some(descs) = s.away_team.situation_descriptions.as_deref() {
            let parts: Vec<String> = descs
                .iter()
                .map(|d| match d {
                    SituationDesc::PP => match &s.time_remaining {
                        Some(t) => format!("PP: {}", t),
                        None => "PP".to_string(),
                    },
                    SituationDesc::EN => "EN".to_string(),
                    SituationDesc::Unknown => "Unknown".to_string(),
                })
                .collect();

            if !parts.is_empty() {
                let label = format!("[{}] ", parts.join(", "));
                left_spans.push(Span::styled(label, Style::new().fg(AWAY_BAR_COLOR).bold()));
            }
        }
    }
    left_spans.push(Span::styled(
        &game.away_team.name.default,
        name_style(game.away_team.abbrev),
    ));
    frame.render_widget(Line::from(left_spans).right_aligned(), chunks[0]);
    frame.render_widget(Line::from("vs").centered(), chunks[1]);

    let mut right_spans = vec![];
    right_spans.push(Span::styled(
        &game.home_team.name.default,
        name_style(game.home_team.abbrev),
    ));
    if let Some(s) = situation {
        if let Some(descs) = s.home_team.situation_descriptions.as_deref() {
            let parts: Vec<String> = descs
                .iter()
                .map(|d| match d {
                    SituationDesc::PP => match &s.time_remaining {
                        Some(t) => format!("PP: {}", t),
                        None => "PP".to_string(),
                    },
                    SituationDesc::EN => "EN".to_string(),
                    SituationDesc::Unknown => "Unknown".to_string(),
                })
                .collect();
            if !parts.is_empty() {
                let label = format!(" [{}]", parts.join(", "));
                right_spans.push(Span::styled(label, Style::new().fg(AWAY_BAR_COLOR).bold()));
            }
        }
    }
    frame.render_widget(Line::from(right_spans).left_aligned(), chunks[2]);
}

fn render_big_score(game: &GameData, frame: &mut Frame, area: Rect) {
    let chunks = split_info_left_middle_right(area, MIDDLE_LENGTH);

    let away_score = build_big_text(
        game.away_team.score.unwrap_or(0).to_string(),
        Alignment::Right,
    );
    frame.render_widget(away_score, chunks[0]);
    let dash = build_big_text("-".to_string(), Alignment::Center);
    frame.render_widget(dash, chunks[1]);
    let home_score = build_big_text(
        game.home_team.score.unwrap_or(0).to_string(),
        Alignment::Left,
    );
    frame.render_widget(home_score, chunks[2]);
}

fn build_big_text(text: String, alignment: Alignment) -> BigText<'static> {
    BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::new().fg(BIG_SCORE_COLOR))
        .lines(vec![Line::from(text)])
        .alignment(alignment)
        .build()
}

fn render_shots_on_goal(game: &GameData, frame: &mut Frame, area: Rect) {
    let chunks = split_info_left_middle_right(area, MIDDLE_LENGTH);
    frame.render_widget(
        create_line_from_sog(game.away_team.sog.unwrap_or(0), Alignment::Right),
        chunks[0],
    );
    frame.render_widget(
        create_line_from_sog(game.home_team.sog.unwrap_or(0), Alignment::Left),
        chunks[2],
    );
}

fn create_line_from_sog(sog: u16, alignment: Alignment) -> Line<'static> {
    Line::from(format!("SOG: {}", sog))
        .style(Style::new().fg(Color::DarkGray))
        .alignment(alignment)
}

fn render_season_records(
    game_story: Option<&crate::models::games::game_story::GameStoryResponse>,
    frame: &mut Frame,
    area: Rect,
) {
    let chunks = split_info_left_middle_right(area, MIDDLE_LENGTH);
    let grey = Style::new().fg(Color::DarkGray);

    let record = |team: Option<&crate::models::games::game_story::StoryTeam>| {
        team.and_then(|t| t.record.clone()).unwrap_or_default()
    };
    let away = record(game_story.and_then(|gs| gs.away_team.as_ref()));
    let home = record(game_story.and_then(|gs| gs.home_team.as_ref()));

    frame.render_widget(
        Line::from(away).style(grey).alignment(Alignment::Right),
        chunks[0],
    );
    frame.render_widget(
        Line::from(home).style(grey).alignment(Alignment::Left),
        chunks[2],
    );
}

pub fn get_period_title(period: &PeriodDescriptor) -> String {
    match period.period_type {
        PeriodType::REG => format!("{} Period", ordinal(period.number as u16)),
        PeriodType::OT => match period.ot_periods.unwrap_or(0) {
            0 | 1 => "Overtime".to_string(),
            n => format!("{} Overtime", ordinal(n as u16)),
        },
        PeriodType::SO => "Shootout".to_string(),
        _ => "Unknown Period".to_string(),
    }
}

/// Format a 1-based number as an ordinal string, e.g. `1 -> "1st"`, `12 -> "12th"`.
pub fn ordinal(n: u16) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn render_series_info(series: &SeriesStatus, frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Line::from(format!(
            "{} - Game {}",
            series.series_abbrev, series.game_number_of_series
        ))
        .style(Style::new().fg(Color::DarkGray))
        .centered(),
        area,
    );
}

fn render_series_status(
    series: &SeriesStatus,
    total_goals: Option<&str>,
    frame: &mut Frame,
    area: Rect,
) {
    if series.needed_to_win == 0 {
        if let Some(status) = total_goals {
            frame.render_widget(
                Line::from(status.to_string())
                    .style(Style::new().fg(Color::DarkGray))
                    .centered(),
                area,
            );
        }
        return;
    }
    // If series is tied
    let line = if series.top_seed_wins == series.bottom_seed_wins {
        Line::from(format!("Series tied {0} - {0}", series.top_seed_wins))
    }
    // If top seed won the series
    else if series.top_seed_wins == series.needed_to_win {
        Line::from(format!(
            "{} wins {} - {}",
            series.top_seed_team_abbrev, series.top_seed_wins, series.bottom_seed_wins
        ))
    }
    // If bottom seed won the series
    else if series.bottom_seed_wins == series.needed_to_win {
        Line::from(format!(
            "{} wins {} - {}",
            series.bottom_seed_team_abbrev, series.bottom_seed_wins, series.top_seed_wins
        ))
    }
    // Series not over
    else {
        let (leading_team, leading_team_score, trailing_team_score) =
            if series.top_seed_wins > series.bottom_seed_wins {
                (
                    series.top_seed_team_abbrev,
                    series.top_seed_wins,
                    series.bottom_seed_wins,
                )
            } else {
                (
                    series.bottom_seed_team_abbrev,
                    series.bottom_seed_wins,
                    series.top_seed_wins,
                )
            };
        Line::from(format!(
            "{} leads {} - {}",
            leading_team, leading_team_score, trailing_team_score
        ))
    };
    frame.render_widget(
        line.style(Style::new().fg(Color::DarkGray)).centered(),
        area,
    );
}

/// Build the status line for a two-game total-goals series by aggregating goals
fn total_goals_status_line(series: &SeriesResponse, viewed_game_number: usize) -> String {
    use crate::models::games::games::GameState;

    let top_abbrev = series.top_seed_team.abbrev;
    let bottom_abbrev = series.bottom_seed_team.abbrev;

    let mut top_goals: u32 = 0;
    let mut bottom_goals: u32 = 0;
    let mut viewed_game_ended = false;
    for g in &series.games {
        if g.game_number as usize > viewed_game_number {
            continue;
        }
        if g.game_number as usize == viewed_game_number {
            viewed_game_ended = matches!(
                g.game_state,
                GameState::OFF | GameState::FINAL | GameState::OVER
            );
        }
        for team in [&g.home_team, &g.away_team] {
            let score = team.score.unwrap_or(0) as u32;
            if team.abbrev == top_abbrev {
                top_goals += score;
            } else if team.abbrev == bottom_abbrev {
                bottom_goals += score;
            }
        }
    }

    let is_final = viewed_game_number as u8 == series.length && viewed_game_ended;

    if top_goals == bottom_goals {
        return format!("Series tied {0}G - {0}G", top_goals);
    }

    let (leader, leader_goals, trailer_goals) = if top_goals > bottom_goals {
        (top_abbrev, top_goals, bottom_goals)
    } else {
        (bottom_abbrev, bottom_goals, top_goals)
    };

    let verb = if is_final { "wins" } else { "leads" };
    format!("{} {} {}G - {}G", leader, verb, leader_goals, trailer_goals)
}

// Helper to create the areas for left-center-right
pub fn split_info_left_middle_right(area: Rect, middle_length: u16) -> Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(middle_length),
            Constraint::Fill(1),
        ])
        .split(area)
}

/// The content area and visible line range produced by [`render_scroll_frame`].
pub struct ScrollView {
    /// Inner content area (between the scroll indicators).
    pub content: Rect,
    /// Range of line indices currently visible: `offset..end`.
    pub range: std::ops::Range<usize>,
}

/// Reserve top/bottom rows for `▲`/`▼` scroll indicators, compute the visible
/// window over `total_lines`, render the indicators, and update `max_scroll` /
/// `visible_rows`. Shared by the scrollable game views (scoring, stats,
/// pre-game).
pub fn render_scroll_frame(
    frame: &mut Frame,
    area: Rect,
    total_lines: usize,
    scroll_offset: usize,
    max_scroll: &mut usize,
    visible_rows: &mut usize,
) -> ScrollView {
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let content = vert_chunks[1];
    let content_height = content.height as usize;
    *visible_rows = content_height;

    let last_line = total_lines.saturating_sub(content_height);
    *max_scroll = last_line;
    let offset = scroll_offset.min(last_line);
    let end = (offset + content_height).min(total_lines);

    frame.render_widget(
        Line::from(if offset > 0 { "▲" } else { "" }).centered(),
        vert_chunks[0],
    );
    frame.render_widget(
        Line::from(if offset < last_line { "▼" } else { "" }).centered(),
        vert_chunks[2],
    );

    ScrollView {
        content,
        range: offset..end,
    }
}

pub fn get_block_title(focus: &GamesFocus) -> String {
    match focus {
        GamesFocus::Scoring => " Scoring ".to_string(),
        GamesFocus::Boxscore => " Boxscore ".to_string(),
        GamesFocus::Stats => " Game Stats ".to_string(),
        GamesFocus::Pregame => " Pre-Game ".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total_goals_series_json(
        top: &str,
        bottom: &str,
        length: u8,
        games: &[(u8, &str, Option<u8>, &str, Option<u8>, &str)],
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
            .map(|(num, away, as_, home, hs, state)| {
                format!(
                    r#"{{"id":1,"gameNumber":{num},"ifNecessary":false,"venue":{{"default":"v"}},"startTimeUTC":"2020-01-01T00:00:00Z","gameState":"{state}","awayTeam":{away_t},"homeTeam":{home_t}}}"#,
                    num = num,
                    state = state,
                    away_t = team(away, *as_),
                    home_t = team(home, *hs),
                )
            })
            .collect();
        format!(
            r#"{{"round":1,"roundAbbrev":"QF","roundLabel":"Quarterfinals","seriesLetter":"B","neededToWin":0,"length":{length},"bottomSeedTeam":{bottom},"topSeedTeam":{top},"games":[{games}]}}"#,
            length = length,
            top = seed(top),
            bottom = seed(bottom),
            games = games_json.join(","),
        )
    }

    #[test]
    fn total_goals_leader_in_progress() {
        // Only game 1 played: MTL 1 @ CHI 0 -> MTL leads on aggregate.
        let json = total_goals_series_json(
            "CHI",
            "MTL",
            2,
            &[(1, "MTL", Some(1), "CHI", Some(0), "OFF")],
        );
        let series = SeriesResponse::from_json(&json).unwrap();
        // Viewing game 1 (not the last game) -> leads.
        assert_eq!(total_goals_status_line(&series, 1), "MTL leads 1G - 0G");
    }

    #[test]
    fn total_goals_winner_when_series_complete() {
        // Both games played: g1 MTL 1 @ CHI 0, g2 CHI 2 @ MTL 2 ->
        // aggregate MTL 3, CHI 2 -> MTL wins. Viewing game 2 (the last game).
        let json = total_goals_series_json(
            "CHI",
            "MTL",
            2,
            &[
                (1, "MTL", Some(1), "CHI", Some(0), "OFF"),
                (2, "CHI", Some(2), "MTL", Some(2), "OFF"),
            ],
        );
        let series = SeriesResponse::from_json(&json).unwrap();
        assert_eq!(total_goals_status_line(&series, 2), "MTL wins 3G - 2G");
    }

    #[test]
    fn total_goals_game_1_does_not_spoil_game_2() {
        // Both games are already played, but we're viewing game 1. We must only
        // show game 1's result (MTL leads 1 - 0), not the aggregate, and never
        // "wins" since game 1 isn't the series' last game.
        let json = total_goals_series_json(
            "CHI",
            "MTL",
            2,
            &[
                (1, "MTL", Some(1), "CHI", Some(0), "OFF"),
                (2, "CHI", Some(2), "MTL", Some(2), "OFF"),
            ],
        );
        let series = SeriesResponse::from_json(&json).unwrap();
        assert_eq!(total_goals_status_line(&series, 1), "MTL leads 1G - 0G");
    }

    #[test]
    fn total_goals_not_final_until_last_game_ends() {
        // Viewing game 2 while it's live: still "leads", not "wins".
        let json = total_goals_series_json(
            "CHI",
            "MTL",
            2,
            &[
                (1, "MTL", Some(1), "CHI", Some(0), "OFF"),
                (2, "CHI", Some(3), "MTL", Some(0), "LIVE"),
            ],
        );
        let series = SeriesResponse::from_json(&json).unwrap();
        // Aggregate through game 2: CHI 3, MTL 1 -> CHI leads (game 2 not final).
        assert_eq!(total_goals_status_line(&series, 2), "CHI leads 3G - 1G");
    }

    #[test]
    fn total_goals_tie() {
        let json = total_goals_series_json(
            "CHI",
            "MTL",
            2,
            &[
                (1, "MTL", Some(1), "CHI", Some(1), "OFF"),
                (2, "CHI", Some(2), "MTL", Some(2), "OFF"),
            ],
        );
        let series = SeriesResponse::from_json(&json).unwrap();
        assert_eq!(total_goals_status_line(&series, 2), "Series tied 3G - 3G");
    }
}
