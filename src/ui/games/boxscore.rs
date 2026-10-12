use crate::app::App;
use crate::models::games::boxscore::{BoxscoreResponse, Defenseman, Forward, Goalie, PlayerData};
use crate::state::games_state::{BoxscorePosition, BoxscoreTeam};
use crate::ui::render::border_style;

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Row, Table},
};

const BOXSCORE_FORWARDS_COLUMNS: [&str; 17] = [
    "#", "Forwards", "POS", "G", "A", "P", "+/-", "PIM", "TOI", "SHIFT", "PPG", "S", "BLK", "HITS",
    "GV", "TK", "FO%",
];
const BOXSCORE_DEFENSE_COLUMNS: [&str; 15] = [
    "#",
    "Defensemen",
    "G",
    "A",
    "P",
    "+/-",
    "PIM",
    "TOI",
    "SHIFT",
    "PPG",
    "S",
    "BLK",
    "HITS",
    "GV",
    "TK",
];
const BOXSCORE_GOALIES_COLUMNS: [&str; 11] = [
    "#", "Goalies", "SA", "SV", "GA", "EV", "PP", "SH", "SV%", "PIM", "TOI",
];

const BOXSCORE_FORWARDS_COLUMN_WIDTHS: [Constraint; 17] = [
    Constraint::Length(3), // #
    Constraint::Min(20),   // Name
    Constraint::Length(4), // POS
    Constraint::Length(3), // G
    Constraint::Length(3), // A
    Constraint::Length(3), // P
    Constraint::Length(4), // +/-
    Constraint::Length(4), // PIM
    Constraint::Length(7), // TOI
    Constraint::Length(6), // SHIFT
    Constraint::Length(4), // PPG
    Constraint::Length(3), // S
    Constraint::Length(4), // BLK
    Constraint::Length(6), // HITS
    Constraint::Length(3), // GV
    Constraint::Length(3), // TK
    Constraint::Length(5), // FO%
];

const BOXSCORE_DEFENSE_COLUMN_WIDTHS: [Constraint; 15] = [
    Constraint::Length(3), // #
    Constraint::Min(20),   // Name
    Constraint::Length(3), // G
    Constraint::Length(3), // A
    Constraint::Length(3), // P
    Constraint::Length(4), //+/-
    Constraint::Length(4), // PIM
    Constraint::Length(7), // TOI
    Constraint::Length(6), // SHIFT
    Constraint::Length(4), // PPG
    Constraint::Length(3), // S
    Constraint::Length(5), // BLK
    Constraint::Length(6), // HITS
    Constraint::Length(4), // GV
    Constraint::Length(4), // TK
];

const BOXSCORE_GOALIES_COLUMN_WIDTHS: [Constraint; 11] = [
    Constraint::Length(3), // #
    Constraint::Min(20),   // Name
    Constraint::Length(4), // SA
    Constraint::Length(4), // SV
    Constraint::Length(4), // GA
    Constraint::Length(6), // EV
    Constraint::Length(5), // PP
    Constraint::Length(5), // SH
    Constraint::Length(5), // SV%
    Constraint::Length(4), // PIM
    Constraint::Length(6), // TOI
];

pub fn render_boxscore(frame: &mut Frame, app: &mut App, area: Rect) {
    // Pass visible rows to standings state
    app.state.games.visible_rows = area.height.saturating_sub(3) as usize;

    let is_home = matches!(&app.state.games.boxscore_selected_team, BoxscoreTeam::Home);
    let game_id = app
        .state
        .games
        .games_data
        .as_ref()
        .and_then(|g| g.games.get(app.state.games.selected_game_index))
        .map(|g| g.id);
    let boxscore = game_id.and_then(|id| app.state.games.boxscore_data.get(&id));

    if let Some(boxscore) = boxscore {
        let rows = map_rows(
            boxscore,
            is_home,
            &app.state.games.boxscore_selected_position,
        );
        let (widths, header): (&[Constraint], &[&str]) =
            match &app.state.games.boxscore_selected_position {
                BoxscorePosition::Forwards => {
                    (&BOXSCORE_FORWARDS_COLUMN_WIDTHS, &BOXSCORE_FORWARDS_COLUMNS)
                }
                BoxscorePosition::Defensemen => {
                    (&BOXSCORE_DEFENSE_COLUMN_WIDTHS, &BOXSCORE_DEFENSE_COLUMNS)
                }
                BoxscorePosition::Goalies => {
                    (&BOXSCORE_GOALIES_COLUMN_WIDTHS, &BOXSCORE_GOALIES_COLUMNS)
                }
            };
        let table = create_table(rows, get_boxscore_title(is_home, boxscore), widths, header);
        frame.render_stateful_widget(table, area, &mut app.state.games.boxscore_table_state);
    } else {
        let missing = app.state.games.missing_text(
            &app.state.games.boxscore_unavailable,
            "Loading boxscore...",
            "No boxscore available for this game.",
        );
        frame.render_widget(Line::from(missing).centered(), area);
    }
}

fn map_forwards_rows(players: &[Forward]) -> Vec<Row<'static>> {
    players
        .iter()
        .map(|p| {
            let name = p.name.default.clone();
            Row::new(vec![
                p.sweater_number.to_string(),
                name,
                p.position.to_string(),
                p.goals.to_string(),
                p.assists.to_string(),
                p.points.to_string(),
                p.plus_minus.to_string(),
                p.pim.unwrap_or(0).to_string(),
                p.toi
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.shifts.to_string(),
                p.power_play_goals.to_string(),
                p.sog.to_string(),
                p.blocked_shots.to_string(),
                p.hits.to_string(),
                p.giveaways
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.takeaways
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.faceoff_winning_pctg
                    .filter(|f| *f > 0.0)
                    .map(|f| format!("{:.1}", f * 100.0))
                    .unwrap_or_else(|| "--".to_string()),
            ])
        })
        .collect()
}

fn map_defensemen_rows(players: &[Defenseman]) -> Vec<Row<'static>> {
    players
        .iter()
        .map(|p| {
            let name = p.name.default.clone();
            Row::new(vec![
                p.sweater_number.to_string(),
                name,
                p.goals.to_string(),
                p.assists.to_string(),
                p.points.to_string(),
                p.plus_minus.to_string(),
                p.pim.unwrap_or(0).to_string(),
                p.toi
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.shifts.to_string(),
                p.power_play_goals.to_string(),
                p.sog.to_string(),
                p.blocked_shots.to_string(),
                p.hits.to_string(),
                p.giveaways
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.takeaways
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
            ])
        })
        .collect()
}

fn map_goalie_rows(players: &[Goalie]) -> Vec<Row<'static>> {
    players
        .iter()
        .map(|p| {
            Row::new(vec![
                p.sweater_number.to_string(),
                p.name.default.clone(),
                p.shots_against
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.saves
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.goals_against
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.even_strength_shots_against
                    .clone()
                    .unwrap_or_else(|| "--".to_string()),
                p.power_play_shots_against
                    .clone()
                    .unwrap_or_else(|| "--".to_string()),
                p.shorthanded_shots_against
                    .clone()
                    .unwrap_or_else(|| "--".to_string()),
                p.save_pctg
                    .map(|f| format!("{:.4}", f))
                    .unwrap_or_else(|| "--".to_string()),
                p.pim
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
                p.toi
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "--".to_string()),
            ])
        })
        .collect()
}

fn get_player_data(data: &BoxscoreResponse, is_home: bool) -> Option<&PlayerData> {
    data.player_by_game_stats
        .as_ref()
        .map(|s| if is_home { &s.home_team } else { &s.away_team })
}

pub fn map_rows(
    data: &BoxscoreResponse,
    is_home: bool,
    position: &BoxscorePosition,
) -> Vec<Row<'static>> {
    let Some(players) = get_player_data(data, is_home) else {
        return vec![];
    };

    match position {
        BoxscorePosition::Forwards => map_forwards_rows(&players.forwards),
        BoxscorePosition::Defensemen => map_defensemen_rows(&players.defense),
        BoxscorePosition::Goalies => map_goalie_rows(&players.goalies),
    }
}

fn create_table<'a>(
    rows: Vec<Row<'a>>,
    title: String,
    widths: &[Constraint],
    header: &[&str],
) -> Table<'a> {
    Table::new(rows, widths)
        .block(Block::bordered().title(title).border_style(border_style()))
        .header(
            Row::new(header.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                .style(Style::new().bold().add_modifier(Modifier::UNDERLINED)),
        )
        .column_spacing(1)
        .row_highlight_style(Style::new().bg(Color::DarkGray).bold())
        .highlight_symbol(">> ")
}

fn get_boxscore_title(is_home: bool, boxscore: &BoxscoreResponse) -> String {
    if is_home {
        " ".to_string() + &boxscore.home_team.common_name.default + " "
    } else {
        " ".to_string() + &boxscore.away_team.common_name.default + " "
    }
}
