use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::models::games::game_story::{
    GameStoryResponse, GoalieCompare, PreGameMatchup, StoryTeam,
};
use crate::ui::games::games::{ordinal, render_scroll_frame, split_info_left_middle_right};
use crate::ui::render::border_style;
use crate::ui::render::{ACCENT_BLUE, ACCENT_RED};

const MIDDLE_LENGTH: u16 = 22;

/// A single scrollable row in the pre-game view.
enum Row<'a> {
    /// Three-column row: away value (left), middle (bar/label), home value (right).
    Columns {
        away: Line<'a>,
        middle: Line<'a>,
        home: Line<'a>,
    },
    /// One full-width row (centered).
    Full(Line<'a>),
}

/// Render the pre-game matchup shown before a game starts. Scrolls vertically
/// using the same offset/max_scroll mechanism as the scoring view.
pub fn render_pregame(
    game_story: Option<&GameStoryResponse>,
    frame: &mut Frame,
    area: Rect,
    scroll_offset: usize,
    max_scroll: &mut usize,
    visible_rows: &mut usize,
    missing_text: &str,
) {
    let Some(story) = game_story else {
        *max_scroll = 0;
        *visible_rows = 0;
        frame.render_widget(Line::from(missing_text).centered(), area);
        return;
    };
    // The story loaded but has no matchup section (e.g. some special events)
    let Some(matchup) = story.pre_game_matchup.as_ref() else {
        *max_scroll = 0;
        *visible_rows = 0;
        frame.render_widget(
            Line::from("No pre-game matchup available for this game.").centered(),
            area,
        );
        return;
    };

    let mut rows: Vec<Row> = vec![];
    build_leaders(matchup, &mut rows);
    build_team_stats(matchup, &mut rows);
    build_goalies(
        matchup,
        story.away_team.as_ref(),
        story.home_team.as_ref(),
        &mut rows,
    );

    // Reserve indicator rows, compute the visible window, and render ▲/▼.
    let view = render_scroll_frame(
        frame,
        area,
        rows.len(),
        scroll_offset,
        max_scroll,
        visible_rows,
    );
    let content = view.content;

    // Render the visible window one row at a time so columned and full-width
    // rows can coexist under a single scroll offset.
    for (i, row) in rows[view.range].iter().enumerate() {
        let row_area = Rect {
            x: content.x,
            y: content.y + i as u16,
            width: content.width,
            height: 1,
        };
        match row {
            Row::Full(line) => frame.render_widget(Paragraph::new(line.clone()), row_area),
            Row::Columns { away, middle, home } => {
                let chunks = split_info_left_middle_right(row_area, MIDDLE_LENGTH);
                frame.render_widget(Paragraph::new(away.clone()), chunks[0]);
                frame.render_widget(Paragraph::new(middle.clone()), chunks[1]);
                frame.render_widget(Paragraph::new(home.clone()), chunks[2]);
            }
        }
    }
}

/// A blank spacer row.
fn spacer() -> Row<'static> {
    Row::Full(Line::default())
}

/// An underlined, centered full-width section header.
fn section_header(title: &str) -> Row<'static> {
    Row::Full(
        Line::from(title.to_string())
            .style(border_style().underlined())
            .centered(),
    )
}

/// A three-column row: away value (right), centered label, home value (left).
fn columns_row(away: String, middle: Line<'static>, home: String) -> Row<'static> {
    Row::Columns {
        away: Line::from(away).right_aligned(),
        middle,
        home: Line::from(home).left_aligned(),
    }
}

fn build_leaders(matchup: &PreGameMatchup, rows: &mut Vec<Row<'static>>) {
    rows.push(section_header("Stat Leaders"));
    let grey = Style::new().fg(Color::DarkGray);
    for cat in &matchup.skating_leaders.leaders {
        let away_str = cat
            .away_leader
            .as_ref()
            .map(|l| format!("{} • {}", l.name, l.value))
            .unwrap_or_else(|| "-".to_string());
        let home_str = cat
            .home_leader
            .as_ref()
            .map(|l| format!("{} • {}", l.value, l.name))
            .unwrap_or_else(|| "-".to_string());
        let away_sub = cat
            .away_leader
            .as_ref()
            .map(|l| format!("#{} • {}", sweater(l.sweater_number), l.position_code))
            .unwrap_or_default();
        let home_sub = cat
            .home_leader
            .as_ref()
            .map(|l| format!("#{} • {}", sweater(l.sweater_number), l.position_code))
            .unwrap_or_default();
        let away_val = cat
            .away_leader
            .as_ref()
            .map(|l| l.value as f64)
            .unwrap_or(0.0);
        let home_val = cat
            .home_leader
            .as_ref()
            .map(|l| l.value as f64)
            .unwrap_or(0.0);

        // Category label row.
        rows.push(Row::Full(
            Line::from(pretty_category(&cat.category)).centered(),
        ));
        // Values + bar row.
        rows.push(columns_row(
            away_str,
            compute_bar(away_val, home_val),
            home_str,
        ));
        // Grey subtitle row.
        rows.push(Row::Columns {
            away: Line::from(away_sub).right_aligned().style(grey),
            middle: Line::default(),
            home: Line::from(home_sub).left_aligned().style(grey),
        });
    }
}

fn build_team_stats(matchup: &PreGameMatchup, rows: &mut Vec<Row<'static>>) {
    let a = &matchup.team_season_stats.away_team;
    let h = &matchup.team_season_stats.home_team;
    let grey = Style::new().fg(Color::DarkGray);

    rows.push(section_header("Team Stats"));

    let mut stat = |label: &str,
                    away_value: String,
                    home_value: String,
                    away_rank: u16,
                    home_rank: u16,
                    away_num: f64,
                    home_num: f64| {
        rows.push(Row::Full(Line::from(label.to_string()).centered()));
        rows.push(columns_row(
            away_value,
            compute_bar(away_num, home_num),
            home_value,
        ));
        rows.push(Row::Columns {
            away: Line::from(ordinal(away_rank)).right_aligned().style(grey),
            middle: Line::default(),
            home: Line::from(ordinal(home_rank)).left_aligned().style(grey),
        });
    };

    stat(
        "Power Play %",
        pctg(a.pp_pctg),
        pctg(h.pp_pctg),
        a.pp_pctg_rank,
        h.pp_pctg_rank,
        a.pp_pctg,
        h.pp_pctg,
    );
    stat(
        "Penalty Kill %",
        pctg(a.pk_pctg),
        pctg(h.pk_pctg),
        a.pk_pctg_rank,
        h.pk_pctg_rank,
        a.pk_pctg,
        h.pk_pctg,
    );
    stat(
        "Face-off %",
        pctg(a.faceoff_winning_pctg),
        pctg(h.faceoff_winning_pctg),
        a.faceoff_winning_pctg_rank,
        h.faceoff_winning_pctg_rank,
        a.faceoff_winning_pctg,
        h.faceoff_winning_pctg,
    );
    stat(
        "GF/GP",
        format!("{:.2}", a.goals_for_per_game_played),
        format!("{:.2}", h.goals_for_per_game_played),
        a.goals_for_per_game_played_rank,
        h.goals_for_per_game_played_rank,
        a.goals_for_per_game_played,
        h.goals_for_per_game_played,
    );
    stat(
        "GA/GP",
        format!("{:.2}", a.goals_against_per_game_played),
        format!("{:.2}", h.goals_against_per_game_played),
        a.goals_against_per_game_played_rank,
        h.goals_against_per_game_played_rank,
        a.goals_against_per_game_played,
        h.goals_against_per_game_played,
    );
}

fn build_goalies(
    matchup: &PreGameMatchup,
    away_team: Option<&StoryTeam>,
    home_team: Option<&StoryTeam>,
    rows: &mut Vec<Row<'static>>,
) {
    rows.push(section_header("Goaltending"));

    // Only include goalies who have actually played this season.
    let played = |g: &&GoalieCompare| g.games_played.unwrap_or(0) > 0;

    let team_abbrev = |t: Option<&StoryTeam>| match t.and_then(|team| team.abbrev) {
        Some(abbrev) => abbrev.to_string(),
        None => "?".to_string(),
    };

    // Away team goalies, then home team goalies, centered
    for (idx, (team, goalies)) in [
        (team_abbrev(away_team), &matchup.goalie_comparison.away_team),
        (team_abbrev(home_team), &matchup.goalie_comparison.home_team),
    ]
    .into_iter()
    .enumerate()
    {
        // Separate teams with a blank row (none before the first).
        if idx > 0 {
            rows.push(spacer());
        }
        // Underlined team-abbrev header
        rows.push(Row::Full(
            Line::from(team.to_string())
                .style(Style::new().underlined())
                .centered(),
        ));
        for g in goalies.iter().filter(played) {
            rows.push(Row::Full(Line::from(goalie_summary(g)).centered()));
        }
    }
}

/// One-line goalie summary with fixed-width fields so rows line up vertically.
/// Layout: `Name  GP  W-L-OTL   GAA  SV%  SO`.
fn goalie_summary(g: &GoalieCompare) -> String {
    format!(
        "{:<16}{:>2}GP {:>8} {:>4}GAA {:>4}SV% {:>2}SO",
        g.name.default,
        g.games_played.unwrap_or(0),
        g.record.as_deref().unwrap_or("-"),
        format!("{:.2}", g.gaa.unwrap_or(0.0)),
        // Save percentage shown without the leading zero, e.g. ".902".
        format!("{:.3}", g.save_pctg.unwrap_or(0.0))
            .trim_start_matches('0')
            .to_string(),
        g.shutouts.unwrap_or(0),
    )
}

/// A proportional away/home bar filling `MIDDLE_LENGTH`.
fn compute_bar(mut away: f64, mut home: f64) -> Line<'static> {
    // Shift both values above zero if either is negative, so the ratio is valid.
    let min = away.min(home);
    if min < 0.0 {
        away -= min;
        home -= min;
    }

    let away_zero = away <= 0.0;
    let home_zero = home <= 0.0;

    let (away_length, home_length) = if away_zero && home_zero {
        ((MIDDLE_LENGTH - 3) / 2, (MIDDLE_LENGTH - 3) / 2)
    } else if away_zero {
        (0, MIDDLE_LENGTH - 2)
    } else if home_zero {
        (MIDDLE_LENGTH - 2, 0)
    } else {
        let total = (MIDDLE_LENGTH - 3) as f64;
        let sum = away + home;
        let away_len = ((away / sum) * total).round().max(1.0) as u16;
        (away_len, MIDDLE_LENGTH - 3 - away_len)
    };

    let gap = std::iter::once(Span::raw(if away_zero ^ home_zero { "" } else { " " }));
    let away_spans = std::iter::repeat_n(
        Span::styled("─", Style::new().fg(ACCENT_RED)),
        away_length as usize,
    );
    let home_spans = std::iter::repeat_n(
        Span::styled("─", Style::new().fg(ACCENT_BLUE)),
        home_length as usize,
    );
    let spans: Vec<_> = away_spans.chain(gap).chain(home_spans).collect();

    Line::from(spans).centered()
}

/// Format an optional sweater number
fn sweater(number: Option<u16>) -> String {
    number.map_or_else(|| "  ".to_string(), |n| n.to_string())
}

/// Format a 0..1 ratio as a whole-number percentage.
fn pctg(v: f64) -> String {
    format!("{}%", (v * 100.0).round() as i64)
}

/// Human-friendly names for the skating leader categories.
fn pretty_category(category: &str) -> String {
    match category {
        "points" => "Points",
        "goals" => "Goals",
        "assists" => "Assists",
        "plusMinus" => "+/-",
        other => other,
    }
    .to_string()
}
