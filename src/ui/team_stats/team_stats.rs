use crate::app::App;
use crate::models::team_stats::{Goalie, Skater};
use crate::state::team_stats::team_stats_state::{GameType, PlayerType};
use crate::ui::render::border_style;

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Row, Table},
};

const SKATERS_COLUMNS: [&str; 17] = [
    "Skater", "POS", "GP", "G", "A", "P", "+/-", "PIM", "PPG", "SHG", "GWG", "OTG", "S", "S%",
    "TOI/G", "SHIFT/G", "FO%",
];

const GOALIES_COLUMNS: [&str; 18] = [
    "Goalie", "GP", "GS", "W", "L", "T", "OTL", "GAA", "SV%", "SA", "SV", "GA", "SO", "G", "A",
    "P", "PIM", "TOI",
];

const SKATERS_COLUMNS_WIDTHS: [Constraint; 17] = [
    Constraint::Min(20),   // Name
    Constraint::Length(4), // POS
    Constraint::Length(3), // GP
    Constraint::Length(4), // G
    Constraint::Length(4), // A
    Constraint::Length(4), // P
    Constraint::Length(4), // +/-
    Constraint::Length(4), // PIM
    Constraint::Length(4), // PPG
    Constraint::Length(4), // SHG
    Constraint::Length(4), // GWG
    Constraint::Length(4), // OTG
    Constraint::Length(4), // S
    Constraint::Length(5), // S%
    Constraint::Length(6), // TOI/G
    Constraint::Length(7), // SHIFT/G
    Constraint::Length(5), // FO%
];

const BOXSCORE_GOALIES_COLUMN_WIDTHS: [Constraint; 18] = [
    Constraint::Min(20),   // Name
    Constraint::Length(4), // GP
    Constraint::Length(4), // GS
    Constraint::Length(4), // W
    Constraint::Length(4), // L
    Constraint::Length(3), // T
    Constraint::Length(3), // OTL
    Constraint::Length(7), // GAA
    Constraint::Length(7), // SV%
    Constraint::Length(6), // SA
    Constraint::Length(6), // SV
    Constraint::Length(5), // GA
    Constraint::Length(4), // SO
    Constraint::Length(3), // G
    Constraint::Length(3), // A
    Constraint::Length(3), // P
    Constraint::Length(4), // PIM
    Constraint::Length(8), // TOI
];

pub fn render_team_stats(frame: &mut Frame, app: &mut App, area: Rect) {
    // Pass visible rows to team stats state
    app.state.team_stats.visible_rows = area.height.saturating_sub(3) as usize;
    let show_regular_season = matches!(&app.state.team_stats.game_type, GameType::RegularSeason);

    let title = format!(
        " ({} - {}) {} {} ",
        app.state.date_state.year - 1,
        app.state.date_state.year,
        app.state.team_stats.team_picker.current_team,
        if show_regular_season {
            "Regular Season Stats"
        } else {
            "Playoff Stats"
        }
    );
    let block = Block::bordered().title(title).border_style(border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (team_data, unavailable) = if show_regular_season {
        (
            &app.state.team_stats.regular_season_team_stats_data,
            &app.state.team_stats.regular_season_unavailable,
        )
    } else {
        (
            &app.state.team_stats.playoffs_team_stats_data,
            &app.state.team_stats.playoffs_unavailable,
        )
    };

    if let Some(data) = team_data {
        // Missed playoffs
        if !show_regular_season && data.skaters.is_empty() && data.goalies.is_empty() {
            frame.render_widget(Line::from("⛳").centered(), inner);
            return;
        }

        let show_skaters = matches!(&app.state.team_stats.player_type, PlayerType::Skaters);
        let (rows, widths, header): (Vec<Row<'static>>, &[Constraint], &[&str]) = if show_skaters {
            let rows = map_skater_rows(&data.skaters);
            (rows, &SKATERS_COLUMNS_WIDTHS, &SKATERS_COLUMNS)
        } else {
            let rows = map_goalie_rows(&data.goalies);
            (rows, &BOXSCORE_GOALIES_COLUMN_WIDTHS, &GOALIES_COLUMNS)
        };

        let rows = if rows.is_empty() {
            vec![Row::default().style(Style::default())]
        } else {
            rows
        };

        let table = Table::new(rows, widths)
            .header(
                Row::new(header.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                    .style(Style::new().bold().add_modifier(Modifier::UNDERLINED)),
            )
            .column_spacing(1)
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(table, inner, &mut app.state.team_stats.table_state);
    } else if let Some(hint) = app
        .state
        .team_stats
        .out_of_range
        .as_ref()
        .or(unavailable.as_ref())
    {
        frame.render_widget(
            Line::from(hint.clone())
                .style(Style::new().fg(Color::DarkGray))
                .centered(),
            inner,
        );
    } else {
        frame.render_widget(Line::from("Loading team stats...").centered(), inner);
    }
}

fn map_skater_rows(players: &[Skater]) -> Vec<Row<'static>> {
    players
        .iter()
        .map(|p| {
            let name = p.first_name.default.clone() + " " + &p.last_name.default.clone();
            Row::new(vec![
                name,
                p.position_code.to_string(),
                p.games_played.to_string(),
                p.goals.to_string(),
                p.assists.to_string(),
                p.points.to_string(),
                p.plus_minus
                    .map(|pm| pm.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.penalty_minutes
                    .map(|pm| pm.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.power_play_goals
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.shorthanded_goals
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.game_winning_goals.to_string(),
                p.overtime_goals.to_string(),
                p.shots
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.shooting_pctg
                    .map(|pct| format!("{:.1}", pct * 100.0))
                    .unwrap_or_else(|| "--".to_string()),
                p.avg_time_on_ice_per_game
                    .map(|s| format!("{:.1}", s / 60.0))
                    .unwrap_or_else(|| "--".to_string()),
                p.avg_shifts_per_game
                    .map(|s| format!("{:.1}", s))
                    .unwrap_or_else(|| "--".to_string()),
                p.faceoff_win_pctg
                    .map(|pct| format!("{:.1}", pct * 100.0))
                    .unwrap_or_else(|| "--".to_string()),
            ])
        })
        .collect()
}

fn map_goalie_rows(players: &[Goalie]) -> Vec<Row<'static>> {
    players
        .iter()
        .map(|p| {
            let name = p.first_name.default.clone() + " " + &p.last_name.default.clone();
            let minutes = p.time_on_ice / 60;
            let seconds = p.time_on_ice % 60;
            let toi = format!("{}:{:02}", minutes, seconds);
            Row::new(vec![
                name,
                p.games_played.to_string(),
                p.games_started.to_string(),
                p.wins
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.losses
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.ties
                    .map(|ties| ties.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.overtime_losses
                    .map(|losses| losses.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                format!("{:.2}", p.goals_against_average),
                p.save_percentage
                    .map(|pct| format!("{:.3}", pct))
                    .unwrap_or_else(|| "--".to_string()),
                p.shots_against
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.saves
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.goals_against.to_string(),
                p.shutouts.to_string(),
                p.goals.to_string(),
                p.assists.to_string(),
                p.points.to_string(),
                p.penalty_minutes
                    .map(|pm| pm.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                toi,
            ])
        })
        .collect()
}
