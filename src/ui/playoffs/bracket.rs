use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::models::playoffs::bracket::Series;
use crate::models::playoffs::series::SeriesResponse;
use crate::ui::games::games::BIG_SCORE_COLOR;
use crate::ui::playoffs::series::aggregate_series_goals;
use crate::ui::render::{BORDER_COLOR, border_style};
use crate::{app::App, state::playoffs_state::PlayoffsState};
use std::collections::HashMap;
use tui_big_text::{BigText, PixelSize};

// Base card width used for most brackets.
const CARD_WIDTH: u16 = 19;
// Wider card width used for division-era brackets that show a seed prefix
// (e.g. "WC1 ") on round-1 cards. Uniform across the whole bracket.
const WIDE_CARD_WIDTH: u16 = 22;
const CARD_HEIGHT: u16 = 5;
// Horizontal gap length between rounds
const ROUND_HOR_GAP: u16 = 6;

/// Card width for a given bracket year. Division-era years (2014+, except the
/// 2020 COVID bubble which used conference-wide reseeding) show a seed prefix
/// on round-1 cards, so those brackets use the wider card.
fn card_width_for_year(year: i32) -> u16 {
    if year >= 2014 && year != 2020 {
        WIDE_CARD_WIDTH
    } else {
        CARD_WIDTH
    }
}

const COLOR_WIN: Color = BIG_SCORE_COLOR;
const COLOR_LOSE: Color = Color::DarkGray;

pub fn render_playoffs(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut outer_block = Block::bordered()
        .border_style(border_style())
        .title(format!(
            " {} Stanley Cup Playoffs ",
            app.state.date_state.year
        ));
    // Too long for the menu hints, so it goes on the border like the Games
    // prev/next dates; only when there are series to open
    let has_series = app
        .state
        .playoffs
        .bracket_data
        .as_ref()
        .is_some_and(|b| !b.series.is_empty());
    if has_series {
        outer_block = outer_block.title_top(
            Line::from(" Hint: shift + series letter to open series ")
                .style(Style::new().fg(Color::DarkGray))
                .right_aligned(),
        );
    }

    let inner = outer_block.inner(area);

    frame.render_widget(outer_block, area);

    if let Some(playoff_bracket) = app
        .state
        .playoffs
        .bracket_data
        .as_ref()
        .filter(|b| !b.series.is_empty())
    {
        let h_off = app.state.playoffs.horizontal_scroll_offset as u16;
        let v_off = app.state.playoffs.vertical_scroll_offset as u16;
        let year = app.state.date_state.year;
        let cw = card_width_for_year(year);

        let bracket_area = Rect {
            x: inner.x + 1,
            y: inner.y + 1,
            width: inner.width.saturating_sub(2),
            height: inner.height.saturating_sub(2),
        };
        // Pass visible rows/columns and max scroll to playoff_bracket state
        app.state.playoffs.visible_columns = bracket_area.width.saturating_sub(1) as usize;
        app.state.playoffs.visible_rows = bracket_area.height.saturating_sub(1) as usize;

        app.state.playoffs.horizontal_max_scroll =
            canvas_width(layout_for_year(year), cw).saturating_sub(bracket_area.width) as usize;
        app.state.playoffs.vertical_max_scroll =
            canvas_height(layout_for_year(year)).saturating_sub(bracket_area.height) as usize;

        render_bracket(
            frame,
            bracket_area,
            &playoff_bracket.series,
            &app.state.playoffs.bracket_series_data,
            year,
            h_off,
            v_off,
        );
        render_scroll_indicators(frame, inner, &app.state.playoffs);
    } else if app.state.playoffs.bracket_data.is_some() {
        // Response returned {}
        frame.render_widget(
            Line::from("No data available.")
                .centered()
                .style(Style::new().fg(Color::DarkGray)),
            inner,
        );
    } else if let Some(message) = &app.state.playoffs.bracket_unavailable {
        frame.render_widget(
            Line::from(message.as_str())
                .centered()
                .style(Style::new().fg(Color::DarkGray)),
            inner,
        );
    } else {
        frame.render_widget(Line::from("Loading bracket...").centered(), inner);
    };
}

// Era-specific bracket layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BracketLayout {
    Chain(&'static [&'static str]),
    /// 1929–1942 two-division format
    DoubleDivision,
    /// 1968–1974: eight teams, two divisions
    MirroredTwoRound(MirrorArrangement),
    /// 1975–1979
    ReseededTree,
    /// 16-team bracket
    Modern,
}

/// The letter assignment for a [`BracketLayout::MirroredTwoRound`] bracket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MirrorArrangement {
    left_qf: [&'static str; 2],
    left_sf: &'static str,
    final_series: &'static str,
    right_sf: &'static str,
    right_qf: [&'static str; 2],
}

fn layout_for_year(year: i32) -> BracketLayout {
    match year {
        1920 => BracketLayout::Chain(&["I"]),
        1918 | 1919 | 1921 | 1922 => BracketLayout::Chain(&["A", "I"]),
        1923 | 1924 | 1926 => BracketLayout::Chain(&["A", "I", "M"]),
        1927 | 1928 => BracketLayout::Chain(&["A", "I", "M", "J", "B"]),
        1925 => BracketLayout::Chain(&["A", "M"]),
        1929..=1942 => BracketLayout::DoubleDivision,
        1943..=1967 => BracketLayout::Chain(&["A", "I", "B"]),
        1968..=1970 => BracketLayout::MirroredTwoRound(MirrorArrangement {
            left_qf: ["C", "D"],
            left_sf: "J",
            final_series: "M",
            right_sf: "I",
            right_qf: ["A", "B"],
        }),
        1971..=1974 => BracketLayout::MirroredTwoRound(MirrorArrangement {
            left_qf: ["C", "B"],
            left_sf: "J",
            final_series: "M",
            right_sf: "I",
            right_qf: ["A", "D"],
        }),
        1975..=1979 => BracketLayout::ReseededTree,
        _ => BracketLayout::Modern,
    }
}

// All playoff series letters (A–O)
const ALL_SERIES_LETTERS: [&str; 15] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O",
];

// 6 columns, 4 rows
fn series_letter_to_position(letter: &str) -> Option<(usize, usize)> {
    match letter {
        "A" => Some((6, 0)), // Top right
        "B" => Some((6, 1)),
        "C" => Some((6, 2)),
        "D" => Some((6, 3)),
        "I" => Some((5, 0)),
        "J" => Some((5, 1)),
        "M" => Some((4, 0)),
        "O" => Some((3, 0)), // Stanley Cup Final
        "N" => Some((2, 0)),
        "K" => Some((1, 0)),
        "L" => Some((1, 1)),
        "E" => Some((0, 0)), // Top left
        "F" => Some((0, 1)),
        "G" => Some((0, 2)),
        "H" => Some((0, 3)),
        _ => None,
    }
}

fn canvas_width(layout: BracketLayout, cw: u16) -> u16 {
    match layout {
        BracketLayout::Chain(letters) => {
            let n = letters.len() as u16;
            n * cw + n.saturating_sub(1) * ROUND_HOR_GAP
        }
        BracketLayout::DoubleDivision => 4 * cw + 3 * ROUND_HOR_GAP,
        BracketLayout::MirroredTwoRound(_) => 5 * cw + 4 * ROUND_HOR_GAP,
        BracketLayout::ReseededTree => 7 * cw + 6 * ROUND_HOR_GAP,
        BracketLayout::Modern => 7 * cw + 6 * ROUND_HOR_GAP,
    }
}
fn canvas_height(layout: BracketLayout) -> u16 {
    match layout {
        BracketLayout::Chain(_) => 1 + CARD_HEIGHT,
        BracketLayout::DoubleDivision | BracketLayout::MirroredTwoRound(_) => {
            1 + 2 * CARD_HEIGHT + 1
        }
        BracketLayout::ReseededTree | BracketLayout::Modern => 1 + 4 * CARD_HEIGHT + 3,
    }
}

fn r1_y(row: usize) -> u16 {
    let gap = 1;
    1 + row as u16 * (CARD_HEIGHT + gap)
}
fn midpoint(a: u16, b: u16) -> u16 {
    (a + b) / 2
}

// Compute the card position based on R1 cards
fn card_virtual_pos(col: usize, row: usize, cw: u16) -> (u16, u16) {
    let x = col as u16 * (cw + ROUND_HOR_GAP);

    let y = match col {
        // R1
        0 | 6 => r1_y(row),
        // R2
        1 | 5 => match row {
            0 => midpoint(r1_y(0), r1_y(1)),
            1 => midpoint(r1_y(2), r1_y(3)),
            _ => 0,
        },
        // Conference finals
        2 | 4 => midpoint(midpoint(r1_y(0), r1_y(1)), midpoint(r1_y(2), r1_y(3))),
        // Stanley Cup Final
        3 => midpoint(
            midpoint(midpoint(r1_y(0), r1_y(1)), midpoint(r1_y(2), r1_y(3))),
            midpoint(midpoint(r1_y(0), r1_y(1)), midpoint(r1_y(2), r1_y(3))),
        ),
        _ => 0,
    };

    (x, y)
}

fn card_mid_y(col: usize, row: usize, cw: u16) -> u16 {
    card_virtual_pos(col, row, cw).1 + CARD_HEIGHT / 2
}

fn render_bracket(
    frame: &mut Frame,
    area: Rect,
    series_list: &[Series],
    bracket_series_data: &HashMap<String, SeriesResponse>,
    year: i32,
    h_off: u16,
    v_off: u16,
) {
    let cw = card_width_for_year(year);

    let era = layout_for_year(year);
    match era {
        BracketLayout::Chain(letters) => render_chain_bracket(
            frame,
            area,
            letters,
            series_list,
            bracket_series_data,
            year,
            cw,
            h_off,
            v_off,
        ),
        BracketLayout::DoubleDivision => render_double_division(
            frame,
            area,
            series_list,
            bracket_series_data,
            year,
            cw,
            h_off,
            v_off,
        ),
        BracketLayout::MirroredTwoRound(arr) => render_mirrored_two_round(
            frame,
            area,
            arr,
            series_list,
            bracket_series_data,
            year,
            cw,
            h_off,
            v_off,
        ),
        BracketLayout::ReseededTree => render_reseeded_tree(
            frame,
            area,
            series_list,
            bracket_series_data,
            year,
            cw,
            h_off,
            v_off,
        ),
        BracketLayout::Modern => render_modern_bracket(
            frame,
            area,
            series_list,
            bracket_series_data,
            year,
            cw,
            h_off,
            v_off,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_modern_bracket(
    frame: &mut Frame,
    area: Rect,
    series_list: &[Series],
    bracket_series_data: &HashMap<String, SeriesResponse>,
    year: i32,
    cw: u16,
    h_off: u16,
    v_off: u16,
) {
    render_cup_text(frame, area, cw, h_off, v_off);

    let mut labelled_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for &letter in &ALL_SERIES_LETTERS {
        let Some((col, _)) = series_letter_to_position(letter) else {
            continue;
        };
        if !labelled_cols.insert(col) {
            continue;
        }
        if let Some(series) = series_list.iter().find(|s| s.series_letter == letter)
            && let Some(label) = round_label_for(series, cw)
        {
            let vx = col as u16 * (cw + ROUND_HOR_GAP);
            draw_round_label(frame, area, vx, 0, cw, &label, h_off, v_off);
        }
    }

    // Render a card at every known bracket position: the real series if the
    // data contains it, otherwise an empty placeholder.
    for &letter in &ALL_SERIES_LETTERS {
        let Some((col, row)) = series_letter_to_position(letter) else {
            continue;
        };
        let (vx, vy) = card_virtual_pos(col, row, cw);
        let series = series_list.iter().find(|s| s.series_letter == letter);
        let cached = bracket_series_data.get(letter);
        render_series_card(frame, area, series, cached, year, cw, vx, vy, h_off, v_off);
    }

    // Connectors
    draw_east_connectors(frame, area, cw, h_off, v_off);
    draw_west_connectors(frame, area, cw, h_off, v_off);
}

fn round_label_for(series: &Series, cw: u16) -> Option<String> {
    let title = series.series_title.trim();
    if !title.is_empty() && title.len() as u16 <= cw {
        return Some(title.to_string());
    }
    let abbrev = series.series_abbrev.trim();
    if !abbrev.is_empty() {
        return Some(abbrev.to_string());
    }
    (!title.is_empty()).then(|| title.to_string())
}

#[allow(clippy::too_many_arguments)]
fn render_double_division(
    frame: &mut Frame,
    area: Rect,
    series_list: &[Series],
    bracket_series_data: &HashMap<String, SeriesResponse>,
    year: i32,
    cw: u16,
    h_off: u16,
    v_off: u16,
) {
    let col_vx = |col: u16| col * (cw + ROUND_HOR_GAP);
    // Two card rows
    let row_vy = |row: u16| 1 + row * (CARD_HEIGHT + 1);
    let row_mid = |row: u16| row_vy(row) + CARD_HEIGHT / 2;
    // Single-card columns (A, I, D) sit vertically centered between the two rows.
    let center_vy = (row_vy(0) + row_vy(1)) / 2;
    let center_mid = center_vy + CARD_HEIGHT / 2;

    // Placements: (letter, col, vy).
    let placements: [(&str, u16, u16); 5] = [
        ("A", 0, center_vy),
        ("I", 1, center_vy),
        ("D", 2, center_vy),
        ("B", 3, row_vy(0)),
        ("C", 3, row_vy(1)),
    ];

    // Connectors
    for seg in [(0u16, 1u16), (1, 2)] {
        let (lc, rc) = seg;
        for vx in (col_vx(lc) + cw)..col_vx(rc) {
            draw_char_from_virtual(frame, area, vx, center_mid, h_off, v_off, '─');
        }
    }
    // B/C -> D
    let gap = ROUND_HOR_GAP as i32;
    let src_x = col_vx(3) as i32; // B/C left edge (dir -1: no +cw)
    let dst_x = (col_vx(2) + cw) as i32; // D right edge (dir -1: +cw)
    let mid_x = dst_x + gap / 2;
    let my0 = row_mid(0) as i32;
    let my1 = row_mid(1) as i32;
    let join_y = (my0 + my1) / 2;

    // Horizontals from each source's left edge in to the vertical joiner.
    for x in (mid_x + 1)..src_x {
        draw_char_from_virtual(frame, area, x as u16, my0 as u16, h_off, v_off, '─');
        draw_char_from_virtual(frame, area, x as u16, my1 as u16, h_off, v_off, '─');
    }
    // Vertical joiner between the two source rows.
    for y in (my0 + 1)..my1 {
        draw_char_from_virtual(frame, area, mid_x as u16, y as u16, h_off, v_off, '│');
    }
    // Horizontal from the joiner into the destination at the join height.
    for x in dst_x..mid_x {
        draw_char_from_virtual(frame, area, x as u16, join_y as u16, h_off, v_off, '─');
    }

    // Column labels
    let label_cols: [(&str, u16); 4] = [("A", 0), ("I", 1), ("D", 2), ("B", 3)];
    for (letter, col) in label_cols {
        if let Some(series) = series_list.iter().find(|s| s.series_letter == letter)
            && let Some(label) = round_label_for(series, cw)
        {
            draw_round_label(frame, area, col_vx(col), 0, cw, &label, h_off, v_off);
        }
    }

    for (letter, col, vy) in placements {
        let series = series_list.iter().find(|s| s.series_letter == letter);
        let cached = bracket_series_data.get(letter);
        render_series_card(
            frame,
            area,
            series,
            cached,
            year,
            cw,
            col_vx(col),
            vy,
            h_off,
            v_off,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_reseeded_tree(
    frame: &mut Frame,
    area: Rect,
    series_list: &[Series],
    bracket_series_data: &HashMap<String, SeriesResponse>,
    year: i32,
    cw: u16,
    h_off: u16,
    v_off: u16,
) {
    use std::collections::HashMap as Map;

    if series_list.is_empty() {
        return;
    }

    render_cup_text(frame, area, cw, h_off, v_off);

    let max_round = series_list
        .iter()
        .map(|s| s.playoff_round)
        .max()
        .unwrap_or(0);
    let Some(final_idx) = series_list
        .iter()
        .position(|s| s.playoff_round == max_round)
    else {
        return;
    };

    // (winning team id, round) -> index of the series that team won in that round
    let mut won_in_round: Map<(i32, u8), usize> = Map::new();
    for (i, s) in series_list.iter().enumerate() {
        if let Some(wid) = s.winning_team_id {
            won_in_round.insert((wid, s.playoff_round), i);
        }
    }
    let children = |idx: usize| -> Vec<usize> {
        let s = &series_list[idx];
        if s.playoff_round <= 1 {
            return Vec::new();
        }
        let prev = s.playoff_round - 1;
        [s.top_seed_team.as_ref(), s.bottom_seed_team.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|t| won_in_round.get(&(t.id, prev)).copied())
            .collect()
    };

    fn skeleton_children(letter: &str) -> &'static [&'static str] {
        match letter {
            "O" => &["M", "N"],
            "M" => &["I", "J"],
            "N" => &["K", "L"],
            "I" => &["A", "B"],
            "J" => &["C", "D"],
            "K" => &["E", "F"],
            "L" => &["G", "H"],
            _ => &[],
        }
    }

    let mut slot: Map<usize, &'static str> = Map::new();
    fn assign_slots(
        idx: usize,
        letter: &'static str,
        children: &dyn Fn(usize) -> Vec<usize>,
        slot: &mut Map<usize, &'static str>,
    ) {
        slot.insert(idx, letter);
        let kids = children(idx);
        let slots = skeleton_children(letter);
        for (child_idx, child_letter) in kids.iter().zip(slots.iter()) {
            assign_slots(*child_idx, child_letter, children, slot);
        }
    }
    assign_slots(final_idx, "O", &children, &mut slot);

    let mut occupied: Map<&'static str, usize> = Map::new();
    for (&idx, &letter) in &slot {
        occupied.insert(letter, idx);
    }
    let slot_occupied = |letter: &str| occupied.contains_key(letter);

    // Column labels
    let mut labelled_cols: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (&letter, &idx) in &occupied {
        if let Some((col, _)) = series_letter_to_position(letter)
            && labelled_cols.insert(col)
            && let Some(label) = round_label_for(&series_list[idx], cw)
        {
            let vx = col as u16 * (cw + ROUND_HOR_GAP);
            draw_round_label(frame, area, vx, 0, cw, &label, h_off, v_off);
        }
    }

    let pair = |frame: &mut Frame, srcs: [&str; 2], dst: &str, dir: i32| {
        if !slot_occupied(dst) {
            return;
        }
        let s0 = slot_occupied(srcs[0]);
        let s1 = slot_occupied(srcs[1]);
        match (s0, s1) {
            (true, true) => draw_pair(frame, area, srcs, dst, cw, h_off, v_off, dir),
            (true, false) => draw_single(frame, area, srcs[0], dst, cw, h_off, v_off, dir),
            (false, true) => draw_single(frame, area, srcs[1], dst, cw, h_off, v_off, dir),
            (false, false) => {}
        }
    };
    let straight = |frame: &mut Frame, src: &str, dst: &str| {
        if slot_occupied(src) && slot_occupied(dst) {
            draw_straight(frame, area, src, dst, cw, h_off, v_off);
        }
    };
    // East
    pair(frame, ["A", "B"], "I", -1);
    pair(frame, ["C", "D"], "J", -1);
    pair(frame, ["I", "J"], "M", -1);
    straight(frame, "M", "O");
    // West
    pair(frame, ["E", "F"], "K", 1);
    pair(frame, ["G", "H"], "L", 1);
    pair(frame, ["K", "L"], "N", 1);
    straight(frame, "N", "O");

    for (&letter, &idx) in &occupied {
        let Some((col, row)) = series_letter_to_position(letter) else {
            continue;
        };
        let (vx, vy) = card_virtual_pos(col, row, cw);
        let series = &series_list[idx];
        let cached = bracket_series_data.get(letter);
        render_series_card(
            frame,
            area,
            Some(series),
            cached,
            year,
            cw,
            vx,
            vy,
            h_off,
            v_off,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_mirrored_two_round(
    frame: &mut Frame,
    area: Rect,
    arr: MirrorArrangement,
    series_list: &[Series],
    bracket_series_data: &HashMap<String, SeriesResponse>,
    year: i32,
    cw: u16,
    h_off: u16,
    v_off: u16,
) {
    let col_vx = |col: u16| col * (cw + ROUND_HOR_GAP);
    let row_vy = |row: u16| 1 + row * (CARD_HEIGHT + 1);
    let row_mid = |row: u16| row_vy(row) + CARD_HEIGHT / 2;
    let center_vy = (row_vy(0) + row_vy(1)) / 2;
    let center_mid = center_vy + CARD_HEIGHT / 2;

    // Placements
    let placements: [(&str, u16, u16); 7] = [
        (arr.left_qf[0], 0, row_vy(0)),
        (arr.left_qf[1], 0, row_vy(1)),
        (arr.left_sf, 1, center_vy),
        (arr.final_series, 2, center_vy),
        (arr.right_sf, 3, center_vy),
        (arr.right_qf[0], 4, row_vy(0)),
        (arr.right_qf[1], 4, row_vy(1)),
    ];

    // Connector
    let draw_pair_into = |frame: &mut Frame, src_col: u16, dst_col: u16, dir: i32| {
        let cwi = cw as i32;
        let gap = ROUND_HOR_GAP as i32;
        let src_x = src_col as i32 * (cwi + gap) + if dir == 1 { cwi } else { 0 };
        let dst_x = dst_col as i32 * (cwi + gap) + if dir == 1 { 0 } else { cwi };
        let mid_x = if dir == 1 {
            src_x + gap / 2
        } else {
            dst_x + gap / 2
        };
        let my0 = row_mid(0) as i32;
        let my1 = row_mid(1) as i32;
        let join_y = (my0 + my1) / 2;

        let (hs, he) = if dir == 1 {
            (src_x, mid_x)
        } else {
            (mid_x + 1, src_x)
        };
        for x in hs..he {
            draw_char_from_virtual(frame, area, x as u16, my0 as u16, h_off, v_off, '─');
            draw_char_from_virtual(frame, area, x as u16, my1 as u16, h_off, v_off, '─');
        }
        for y in (my0 + 1)..my1 {
            draw_char_from_virtual(frame, area, mid_x as u16, y as u16, h_off, v_off, '│');
        }
        let (ds, de) = if dir == 1 {
            (mid_x + 1, dst_x)
        } else {
            (dst_x, mid_x)
        };
        for x in ds..de {
            draw_char_from_virtual(frame, area, x as u16, join_y as u16, h_off, v_off, '─');
        }
    };

    // Straight horizontal link between two single-card columns at mid-height.
    let draw_straight_between = |frame: &mut Frame, left_col: u16, right_col: u16| {
        for vx in (col_vx(left_col) + cw)..col_vx(right_col) {
            draw_char_from_virtual(frame, area, vx, center_mid, h_off, v_off, '─');
        }
    };

    // Left QF (col 0) → left SF (col 1): sources on the left (dir +1).
    draw_pair_into(frame, 0, 1, 1);
    // Right QF (col 4) → right SF (col 3): sources on the right (dir -1).
    draw_pair_into(frame, 4, 3, -1);
    // Semifinals → Final (col 2).
    draw_straight_between(frame, 1, 2);
    draw_straight_between(frame, 2, 3);

    // Column labels on the single top row (one per column).
    for (letter, col) in [
        (arr.left_qf[0], 0u16),
        (arr.left_sf, 1),
        (arr.final_series, 2),
        (arr.right_sf, 3),
        (arr.right_qf[0], 4),
    ] {
        if let Some(series) = series_list.iter().find(|s| s.series_letter == letter)
            && let Some(label) = round_label_for(series, cw)
        {
            draw_round_label(frame, area, col_vx(col), 0, cw, &label, h_off, v_off);
        }
    }

    // Cards.
    for (letter, col, vy) in placements {
        let series = series_list.iter().find(|s| s.series_letter == letter);
        let cached = bracket_series_data.get(letter);
        render_series_card(
            frame,
            area,
            series,
            cached,
            year,
            cw,
            col_vx(col),
            vy,
            h_off,
            v_off,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_chain_bracket(
    frame: &mut Frame,
    area: Rect,
    letters: &[&str],
    series_list: &[Series],
    bracket_series_data: &HashMap<String, SeriesResponse>,
    year: i32,
    cw: u16,
    h_off: u16,
    v_off: u16,
) {
    let col_vx = |col: u16| col * (cw + ROUND_HOR_GAP);
    const CARD_VY: u16 = 1; // leave the top virtual row for the column labels
    let mid_vy = CARD_VY + CARD_HEIGHT / 2;

    for (i, letter) in letters.iter().enumerate() {
        let col = i as u16;
        let series = series_list.iter().find(|s| &s.series_letter == letter);

        // Column label
        if let Some(label) = series.and_then(|s| round_label_for(s, cw)) {
            draw_round_label(frame, area, col_vx(col), 0, cw, &label, h_off, v_off);
        }

        if i > 0 {
            let prev_right = col_vx(col) - ROUND_HOR_GAP;
            for vx in prev_right..col_vx(col) {
                draw_char_from_virtual(frame, area, vx, mid_vy, h_off, v_off, '─');
            }
        }

        let cached = bracket_series_data.get(*letter);
        render_series_card(
            frame,
            area,
            series,
            cached,
            year,
            cw,
            col_vx(col),
            CARD_VY,
            h_off,
            v_off,
        );
    }
}

// Virtual rows (from the top of the canvas) where the two big-text banners sit
const CUP_TEXT_TOP_VY: u16 = 3; // upper empty band
const CUP_TEXT_BOTTOM_VY: u16 = 19; // lower empty band

/// Draw the "STANLEY CUP" / "PLAYOFFS" big-text banners centered
fn render_cup_text(frame: &mut Frame, area: Rect, cw: u16, h_off: u16, v_off: u16) {
    // Horizontal center of the SCF column (col 3).
    let center_vx = 3 * (cw + ROUND_HOR_GAP) + cw / 2;

    let style = Style::new().fg(BORDER_COLOR).add_modifier(Modifier::DIM);

    draw_big_text_virtual(
        frame,
        area,
        "STANLEY CUP",
        center_vx,
        CUP_TEXT_TOP_VY,
        style,
        h_off,
        v_off,
    );
    draw_big_text_virtual(
        frame,
        area,
        "PLAYOFFS",
        center_vx,
        CUP_TEXT_BOTTOM_VY,
        style,
        h_off,
        v_off,
    );
}

const QUADRANT_GLYPH_WIDTH: u16 = 4;
const QUADRANT_LINE_HEIGHT: u16 = 4;

#[allow(clippy::too_many_arguments)]
fn draw_big_text_virtual(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    center_vx: u16,
    vy: u16,
    style: Style,
    h_off: u16,
    v_off: u16,
) {
    let vw = text.chars().count() as u16 * QUADRANT_GLYPH_WIDTH;
    let vh = QUADRANT_LINE_HEIGHT;
    if vw == 0 {
        return;
    }
    let vx = center_vx.saturating_sub(vw / 2);

    // Render the banner into a scratch buffer at local origin.
    let scratch_area = Rect::new(0, 0, vw, vh);
    let mut scratch = Buffer::empty(scratch_area);
    let big_text = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(style)
        .lines(vec![Line::from(text)])
        .alignment(Alignment::Left)
        .build();
    big_text.render(scratch_area, &mut scratch);

    // Blit each virtual cell to its mapped screen position, clipping to `area`.
    let dst = frame.buffer_mut();
    for local_y in 0..vh {
        let screen_y = (vy + local_y) as i32 - v_off as i32 + area.y as i32;
        if screen_y < area.y as i32 || screen_y >= (area.y + area.height) as i32 {
            continue;
        }
        for local_x in 0..vw {
            let screen_x = (vx + local_x) as i32 - h_off as i32 + area.x as i32;
            if screen_x < area.x as i32 || screen_x >= (area.x + area.width) as i32 {
                continue;
            }
            let src = scratch.cell((local_x, local_y));
            let (Some(src), Some(target)) = (src, dst.cell_mut((screen_x as u16, screen_y as u16)))
            else {
                continue;
            };
            // Only draw non-blank cells so the banner reads as an overlay and
            // doesn't paint empty space over the background.
            if src.symbol() != " " {
                *target = src.clone();
            }
        }
    }
}

fn draw_round_label(
    frame: &mut Frame,
    area: Rect,
    vx: u16,
    vy: u16,
    width: u16,
    label: &str,
    h_off: u16,
    v_off: u16,
) {
    let ax = vx as i32 - h_off as i32;
    let ay = vy as i32 - v_off as i32;
    if ax + width as i32 <= 0 || ax >= area.width as i32 || ay < 0 || ay >= area.height as i32 {
        return;
    }
    let x = (area.x as i32 + ax).max(area.x as i32) as u16;
    let w = (width as i32 - (0 - ax).max(0))
        .max(0)
        .min((area.x as i32 + area.width as i32 - x as i32).max(0)) as u16;
    if w == 0 {
        return;
    }
    frame.render_widget(
        Line::from(label.to_string()).centered(),
        Rect {
            x,
            y: area.y + ay as u16,
            width: w,
            height: 1,
        },
    );
}

/// Render the series card
/// Computes actual positions based on scrolling offsets
#[allow(clippy::too_many_arguments)]
fn render_series_card(
    frame: &mut Frame,
    area: Rect,
    series: Option<&Series>,
    cached: Option<&SeriesResponse>,
    year: i32,
    cw: u16,
    vx: u16,
    vy: u16,
    h_off: u16,
    v_off: u16,
) {
    let ax = vx as i32 - h_off as i32;
    let ay = vy as i32 - v_off as i32;

    if ax + cw as i32 <= 0
        || ax >= area.width as i32
        || ay + CARD_HEIGHT as i32 <= 0
        || ay >= area.height as i32
    {
        return;
    }

    let left = ax;
    let right = ax + cw as i32;
    let top = ay;
    let bottom = ay + CARD_HEIGHT as i32;

    let mut borders = Borders::empty();
    if left >= 0 {
        borders |= Borders::LEFT;
    }
    if right <= area.width as i32 {
        borders |= Borders::RIGHT;
    }
    if top >= 0 {
        borders |= Borders::TOP;
    }
    if bottom <= area.height as i32 {
        borders |= Borders::BOTTOM;
    }

    let x = (area.x as i32 + ax).max(area.x as i32) as u16;
    let y = (area.y as i32 + ay).max(area.y as i32) as u16;

    let width = (cw as i32 - (0 - ax).max(0))
        .max(0)
        .min((area.x as i32 + area.width as i32 - x as i32).max(0)) as u16;
    let height = (CARD_HEIGHT as i32 - (0 - ay).max(0))
        .max(0)
        .min((area.y as i32 + area.height as i32 - y as i32).max(0)) as u16;

    if width == 0 || height == 0 {
        return;
    }

    let is_final = series.is_some_and(|s| s.series_abbrev.eq_ignore_ascii_case("SCF"));
    let border_block = if is_final {
        Block::bordered()
            .borders(borders)
            .border_style(Style::new().fg(BORDER_COLOR))
    } else {
        Block::bordered().borders(borders)
    };

    frame.render_widget(
        border_block,
        Rect {
            x,
            y,
            width,
            height,
        },
    );

    // Compute inner from VIRTUAL position so text doesn't shift when clipped
    let inner_vx = ax + 1;
    let inner_vy = ay + 1;
    let inner_vw = cw as i32 - 2;
    let inner_vh = CARD_HEIGHT as i32 - 2;

    let inner_x = (area.x as i32 + inner_vx).max(area.x as i32) as u16;
    let inner_y = (area.y as i32 + inner_vy).max(area.y as i32) as u16;

    let inner_w = (inner_vw - (0 - inner_vx).max(0))
        .max(0)
        .min((area.x as i32 + area.width as i32 - inner_x as i32).max(0)) as u16;
    let inner_h = (inner_vh - (0 - inner_vy).max(0))
        .max(0)
        .min((area.y as i32 + area.height as i32 - inner_y as i32).max(0)) as u16;

    if inner_w == 0 || inner_h == 0 {
        return;
    }

    // For a placeholder (no series), only the bordered box is drawn.
    let Some(series) = series else {
        return;
    };

    let top_won = series
        .winning_team_id
        .is_some_and(|id| series.top_seed_team.as_ref().is_some_and(|t| t.id == id));
    let bottom_won = series
        .winning_team_id
        .is_some_and(|id| series.bottom_seed_team.as_ref().is_some_and(|t| t.id == id));

    let top_team = series
        .top_seed_team
        .as_ref()
        .map(|t| t.common_name.default.clone())
        .unwrap_or_default();
    let bottom_team = series
        .bottom_seed_team
        .as_ref()
        .map(|t| t.common_name.default.clone())
        .unwrap_or_default();

    let (top_style, bottom_style) = match (top_won, bottom_won) {
        (true, _) => (
            Style::new().fg(COLOR_WIN).bold(),
            Style::new().fg(COLOR_LOSE),
        ),
        (_, true) => (
            Style::new().fg(COLOR_LOSE),
            Style::new().fg(COLOR_WIN).bold(),
        ),
        _ => (Style::default(), Style::default()),
    };

    let (top_seed_wins, bottom_seed_wins): (Option<String>, Option<String>) =
        match cached.filter(|c| c.needed_to_win == 0) {
            Some(c) if series.top_seed_team.is_some() && series.bottom_seed_team.is_some() => {
                let (cached_top_goals, cached_bottom_goals) = aggregate_series_goals(c);
                let goals_for = |abbrev| {
                    if c.top_seed_team.abbrev == abbrev {
                        cached_top_goals
                    } else if c.bottom_seed_team.abbrev == abbrev {
                        cached_bottom_goals
                    } else {
                        0
                    }
                };
                let top = series.top_seed_team.as_ref().map(|t| goals_for(t.abbrev));
                let bottom = series
                    .bottom_seed_team
                    .as_ref()
                    .map(|t| goals_for(t.abbrev));
                (
                    top.map(|g| format!("{g}G")),
                    bottom.map(|g| format!("{g}G")),
                )
            }
            _ if series.top_seed_team.is_some() && series.bottom_seed_team.is_some() => (
                Some(series.top_seed_wins.to_string()),
                Some(series.bottom_seed_wins.to_string()),
            ),
            _ => (None, None),
        };

    // Seeding labels are only shown for round 1 (division/conference seeding).
    let (top_seed, bottom_seed) = if series.playoff_round == 1 {
        (
            seed_label(
                year,
                &series.series_letter,
                series.top_seed_rank,
                &series.top_seed_rank_abbrev,
            ),
            seed_label(
                year,
                &series.series_letter,
                series.bottom_seed_rank,
                &series.bottom_seed_rank_abbrev,
            ),
        )
    } else {
        (None, None)
    };

    let middle_line = Line::from(series.series_letter.clone())
        .centered()
        .style(Style::new().fg(Color::DarkGray));

    let all_lines = vec![
        build_team_line(
            &top_team,
            top_seed.as_deref(),
            top_seed_wins,
            inner_w,
            top_style,
        ),
        middle_line,
        build_team_line(
            &bottom_team,
            bottom_seed.as_deref(),
            bottom_seed_wins,
            inner_w,
            bottom_style,
        ),
    ];

    // Skip lines that are scrolled off the top so text stays anchored to its virtual position
    let lines_clipped_top = (0 - inner_vy).max(0) as usize;
    let visible_lines: Vec<Line> = all_lines.into_iter().skip(lines_clipped_top).collect();

    frame.render_widget(
        Paragraph::new(visible_lines),
        Rect {
            x: inner_x,
            y: inner_y,
            width: inner_w,
            height: inner_h,
        },
    );
}

fn build_team_line<'a>(
    abbrev: &str,
    seed: Option<&str>,
    value: Option<String>,
    width: u16,
    style: Style,
) -> Line<'a> {
    let value_str = value.unwrap_or_default();
    // Prefix the seed (e.g. "A1", "WC1", or "8") when available.
    let name_str = match seed {
        Some(s) if !s.is_empty() => format!("{s} {abbrev}"),
        _ => abbrev.to_string(),
    };
    let pad = (width as usize).saturating_sub(name_str.len() + value_str.len());
    Line::from(vec![
        Span::styled(name_str, style),
        Span::raw(" ".repeat(pad)),
        Span::styled(value_str, style.add_modifier(Modifier::BOLD)),
    ])
}

/// Division letters
const DIVISION_ORDER: [&str; 4] = ["A", "M", "C", "P"];

fn seed_label(
    year: i32,
    series_letter: &str,
    seed_rank: u8,
    seed_rank_abbrev: &str,
) -> Option<String> {
    match year {
        1968..=1974 => {
            let rank = seed_rank_abbrev.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            if !rank.is_empty() {
                Some(rank.to_string())
            } else if seed_rank > 0 {
                Some(seed_rank.to_string())
            } else {
                None
            }
        }
        1994..=2013 | 2020 => Some(seed_rank.to_string()),
        y if y >= 2014 => {
            if seed_rank_abbrev.starts_with("WC") {
                Some(seed_rank_abbrev.to_string())
            } else {
                // Expect "D1"/"D2"/"D3"; replace the leading "D" with the division letter.
                let division = division_from_series_letter(series_letter)?;
                let rank = seed_rank_abbrev.trim_start_matches(|c: char| c.is_ascii_alphabetic());
                Some(format!("{division}{rank}"))
            }
        }
        _ => None,
    }
}

fn division_from_series_letter(series_letter: &str) -> Option<&'static str> {
    let idx = match series_letter {
        "A" | "B" => 0,
        "C" | "D" => 1,
        "E" | "F" => 2,
        "G" | "H" => 3,
        _ => return None,
    };
    Some(DIVISION_ORDER[idx])
}

fn draw_east_connectors(frame: &mut Frame, area: Rect, cw: u16, h_off: u16, v_off: u16) {
    draw_pair(frame, area, ["A", "B"], "I", cw, h_off, v_off, -1);
    draw_pair(frame, area, ["C", "D"], "J", cw, h_off, v_off, -1);
    draw_pair(frame, area, ["I", "J"], "M", cw, h_off, v_off, -1);
    draw_straight(frame, area, "M", "O", cw, h_off, v_off);
}

fn draw_west_connectors(frame: &mut Frame, area: Rect, cw: u16, h_off: u16, v_off: u16) {
    draw_pair(frame, area, ["E", "F"], "K", cw, h_off, v_off, 1);
    draw_pair(frame, area, ["G", "H"], "L", cw, h_off, v_off, 1);
    draw_pair(frame, area, ["K", "L"], "N", cw, h_off, v_off, 1);
    draw_straight(frame, area, "N", "O", cw, h_off, v_off);
}

/// Connect two series (vertically)
fn draw_pair(
    frame: &mut Frame,
    area: Rect,
    srcs: [&str; 2],
    dst: &str,
    card_w: u16,
    h_off: u16,
    v_off: u16,
    dir: i32, // +1 = right, -1 = left
) {
    let (Some((c0, r0)), Some((c1, r1)), Some((dc, _dr))) = (
        series_letter_to_position(srcs[0]),
        series_letter_to_position(srcs[1]),
        series_letter_to_position(dst),
    ) else {
        return;
    };

    let cw = card_w as i32;
    let gap = ROUND_HOR_GAP as i32;

    let src_x = c0 as i32 * (cw + gap) + if dir == 1 { cw } else { 0 };

    let dst_x = dc as i32 * (cw + gap) + if dir == 1 { 0 } else { cw };

    let mid_x = if dir == 1 {
        src_x + gap / 2
    } else {
        dst_x + gap / 2
    };

    let my0 = card_mid_y(c0, r0, card_w) as i32;
    let my1 = card_mid_y(c1, r1, card_w) as i32;
    let join_y = (my0 + my1) / 2;

    // horizontal from sources to mid
    let (start, end) = if dir == 1 {
        (src_x, mid_x)
    } else {
        (mid_x + 1, src_x)
    };

    for x in start..end {
        draw_char_from_virtual(frame, area, x as u16, my0 as u16, h_off, v_off, '─');
        draw_char_from_virtual(frame, area, x as u16, my1 as u16, h_off, v_off, '─');
    }

    // vertical join
    for y in (my0 + 1)..my1 {
        draw_char_from_virtual(frame, area, mid_x as u16, y as u16, h_off, v_off, '│');
    }

    // horizontal from mid to destination
    let (start, end) = if dir == 1 {
        (mid_x + 1, dst_x)
    } else {
        (dst_x, mid_x)
    };

    for x in start..end {
        draw_char_from_virtual(frame, area, x as u16, join_y as u16, h_off, v_off, '─');
    }
}

/// Draw a straight connector between series
fn draw_straight(
    frame: &mut Frame,
    area: Rect,
    src: &str,
    dst: &str,
    cw: u16,
    h_off: u16,
    v_off: u16,
) {
    let (Some((sc, sr)), Some((dc, _))) = (
        series_letter_to_position(src),
        series_letter_to_position(dst),
    ) else {
        return;
    };
    let src_x = sc as u16 * (cw + ROUND_HOR_GAP);
    let dst_x = dc as u16 * (cw + ROUND_HOR_GAP);
    // pick the correct edge of each card
    let src_edge = if src_x < dst_x {
        src_x + cw // going right
    } else {
        src_x // going left
    };
    let dst_edge = if src_x < dst_x {
        dst_x // entering from left
    } else {
        dst_x + cw // entering from right
    };
    let my = card_mid_y(sc, sr, cw);
    let (start, end) = if src_edge < dst_edge {
        (src_edge, dst_edge)
    } else {
        (dst_edge, src_edge)
    };
    for x in start..end {
        draw_char_from_virtual(frame, area, x, my, h_off, v_off, '─');
    }
}

fn draw_single(
    frame: &mut Frame,
    area: Rect,
    src: &str,
    dst: &str,
    cw: u16,
    h_off: u16,
    v_off: u16,
    dir: i32,
) {
    let (Some((sc, sr)), Some((dc, dr))) = (
        series_letter_to_position(src),
        series_letter_to_position(dst),
    ) else {
        return;
    };
    let cw = cw as i32;
    let gap = ROUND_HOR_GAP as i32;

    let src_x = sc as i32 * (cw + gap) + if dir == 1 { cw } else { 0 };
    let dst_x = dc as i32 * (cw + gap) + if dir == 1 { 0 } else { cw };
    let mid_x = if dir == 1 {
        src_x + gap / 2
    } else {
        dst_x + gap / 2
    };
    let src_mid = card_mid_y(sc, sr, cw as u16) as i32;
    let dst_mid = card_mid_y(dc, dr, cw as u16) as i32;

    let (hs, he) = if dir == 1 {
        (src_x, mid_x)
    } else {
        (mid_x + 1, src_x)
    };
    for x in hs..he {
        draw_char_from_virtual(frame, area, x as u16, src_mid as u16, h_off, v_off, '─');
    }

    let (vs, ve) = if src_mid <= dst_mid {
        (src_mid + 1, dst_mid - 1)
    } else {
        (dst_mid + 1, src_mid - 1)
    };
    for y in vs..=ve {
        draw_char_from_virtual(frame, area, mid_x as u16, y as u16, h_off, v_off, '│');
    }

    let (ps, pe) = if dir == 1 {
        (mid_x + 1, dst_x)
    } else {
        (dst_x, mid_x)
    };
    for x in ps..pe {
        draw_char_from_virtual(frame, area, x as u16, dst_mid as u16, h_off, v_off, '─');
    }
}

/// Draw one character given the virtual position
fn draw_char_from_virtual(
    frame: &mut Frame,
    area: Rect,
    vx: u16,
    vy: u16,
    h_off: u16,
    v_off: u16,
    ch: char,
) {
    let ax = vx as i32 - h_off as i32;
    let ay = vy as i32 - v_off as i32;
    if ax < 0 || ay < 0 || ax >= area.width as i32 || ay >= area.height as i32 {
        return;
    }
    frame.render_widget(
        Line::from(Span::from(ch.to_string())),
        Rect {
            x: area.x + ax as u16,
            y: area.y + ay as u16,
            width: 1,
            height: 1,
        },
    );
}

fn render_scroll_indicators(frame: &mut Frame, area: Rect, playoff_bracket: &PlayoffsState) {
    let mid_x = area.x + area.width / 2;
    let mid_y = area.y + area.height / 2;
    if playoff_bracket.horizontal_scroll_offset > 0 {
        frame.render_widget(
            Line::from("◀"),
            Rect {
                x: area.x,
                y: mid_y,
                width: 1,
                height: 1,
            },
        );
    }
    if playoff_bracket.horizontal_scroll_offset < playoff_bracket.horizontal_max_scroll {
        frame.render_widget(
            Line::from("▶"),
            Rect {
                x: area.x + area.width.saturating_sub(1),
                y: mid_y,
                width: 1,
                height: 1,
            },
        );
    }
    if playoff_bracket.vertical_scroll_offset > 0 {
        frame.render_widget(
            Line::from("▲"),
            Rect {
                x: mid_x,
                y: area.y,
                width: 1,
                height: 1,
            },
        );
    }
    if playoff_bracket.vertical_scroll_offset < playoff_bracket.vertical_max_scroll {
        frame.render_widget(
            Line::from("▼"),
            Rect {
                x: mid_x,
                y: area.y + area.height.saturating_sub(1),
                width: 1,
                height: 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BracketLayout, CARD_WIDTH, MirrorArrangement, canvas_height, canvas_width,
        division_from_series_letter, layout_for_year, seed_label,
    };

    #[test]
    fn early_cup_layouts_are_classified() {
        assert_eq!(layout_for_year(1920), BracketLayout::Chain(&["I"]));
        for y in [1918, 1919, 1921, 1922] {
            assert_eq!(layout_for_year(y), BracketLayout::Chain(&["A", "I"]));
        }
        for y in [1923, 1924, 1926] {
            assert_eq!(layout_for_year(y), BracketLayout::Chain(&["A", "I", "M"]));
        }
        // 1925: NHL Semifinal (A) straight to the Stanley Cup Final (M).
        assert_eq!(layout_for_year(1925), BracketLayout::Chain(&["A", "M"]));
        // 1927–1928: two division ladders meeting at the Cup Final, one line
        // with M centered.
        for y in [1927, 1928] {
            assert_eq!(
                layout_for_year(y),
                BracketLayout::Chain(&["A", "I", "M", "J", "B"])
            );
        }
        // 1929–1942: two-division format with the Final centered.
        for y in [1929, 1931, 1937, 1942] {
            assert_eq!(layout_for_year(y), BracketLayout::DoubleDivision);
        }
        // 1943–1967 (Original Six): flat A I B with the Final centered.
        for y in [1943, 1950, 1967] {
            assert_eq!(layout_for_year(y), BracketLayout::Chain(&["A", "I", "B"]));
        }
        // 1968–1970: QF C,D → SF J (left); QF A,B → SF I (right).
        for y in [1968, 1969, 1970] {
            assert_eq!(
                layout_for_year(y),
                BracketLayout::MirroredTwoRound(MirrorArrangement {
                    left_qf: ["C", "D"],
                    left_sf: "J",
                    final_series: "M",
                    right_sf: "I",
                    right_qf: ["A", "B"],
                })
            );
        }
        // 1971–1974: same shape, different QF→SF pairing (C,B → J; A,D → I).
        for y in [1971, 1972, 1973, 1974] {
            assert_eq!(
                layout_for_year(y),
                BracketLayout::MirroredTwoRound(MirrorArrangement {
                    left_qf: ["C", "B"],
                    left_sf: "J",
                    final_series: "M",
                    right_sf: "I",
                    right_qf: ["A", "D"],
                })
            );
        }
        // 1975–1979: four-round reseeded tree (shape derived from data).
        for y in [1975, 1976, 1977, 1978, 1979] {
            assert_eq!(layout_for_year(y), BracketLayout::ReseededTree);
        }
        // Surrounding years fall back to the modern layout for now.
        assert_eq!(layout_for_year(1917), BracketLayout::Modern);
        assert_eq!(layout_for_year(1980), BracketLayout::Modern);
        assert_eq!(layout_for_year(2024), BracketLayout::Modern);
    }

    #[test]
    fn mirrored_two_round_seeds_are_numeric() {
        // 1968–1974 quarterfinal seeds come from the "D1".."D4" rank abbrev.
        assert_eq!(seed_label(1968, "A", 1, "D1").as_deref(), Some("1"));
        assert_eq!(seed_label(1972, "D", 4, "D4").as_deref(), Some("4"));
    }

    #[test]
    fn chain_canvases_grow_with_length_and_stay_below_modern() {
        let single = canvas_width(BracketLayout::Chain(&["I"]), CARD_WIDTH);
        let two = canvas_width(BracketLayout::Chain(&["A", "I"]), CARD_WIDTH);
        let three = canvas_width(BracketLayout::Chain(&["A", "I", "M"]), CARD_WIDTH);
        let modern = canvas_width(BracketLayout::Modern, CARD_WIDTH);
        assert!(single < two && two < three && three < modern);
        for layout in [
            BracketLayout::Chain(&["I"]),
            BracketLayout::Chain(&["A", "I", "M"]),
        ] {
            assert!(canvas_height(layout) < canvas_height(BracketLayout::Modern));
        }
    }

    #[test]
    fn double_division_canvas_is_four_cols_two_rows() {
        let dd = BracketLayout::DoubleDivision;
        // 4 columns wide, between the 3-chain and modern widths.
        assert_eq!(
            canvas_width(dd, CARD_WIDTH),
            4 * CARD_WIDTH + 3 * super::ROUND_HOR_GAP
        );
        assert!(canvas_width(dd, CARD_WIDTH) < canvas_width(BracketLayout::Modern, CARD_WIDTH));
        // Two card rows, so taller than a single-row chain but shorter than modern.
        assert!(canvas_height(dd) > canvas_height(BracketLayout::Chain(&["A", "I"])));
        assert!(canvas_height(dd) < canvas_height(BracketLayout::Modern));
    }

    #[test]
    fn division_from_letter_follows_fixed_order() {
        assert_eq!(division_from_series_letter("A"), Some("A"));
        assert_eq!(division_from_series_letter("B"), Some("A"));
        assert_eq!(division_from_series_letter("C"), Some("M"));
        assert_eq!(division_from_series_letter("D"), Some("M"));
        assert_eq!(division_from_series_letter("E"), Some("C"));
        assert_eq!(division_from_series_letter("F"), Some("C"));
        assert_eq!(division_from_series_letter("G"), Some("P"));
        assert_eq!(division_from_series_letter("H"), Some("P"));
        assert_eq!(division_from_series_letter("I"), None);
    }

    #[test]
    fn conference_era_shows_numeric_seed() {
        // 1994..=2013: use the numeric seed rank, ignore the abbrev.
        assert_eq!(seed_label(2010, "A", 1, "D1").as_deref(), Some("1"));
        assert_eq!(seed_label(2010, "A", 8, "C8").as_deref(), Some("8"));
        assert_eq!(seed_label(2013, "D", 5, "C5").as_deref(), Some("5"));
    }

    #[test]
    fn covid_2020_falls_back_to_numeric_seed() {
        // 2020 bubble used conference-wide reseeding (ranks up to 12).
        assert_eq!(seed_label(2020, "A", 1, "C4").as_deref(), Some("1"));
        assert_eq!(seed_label(2020, "A", 8, "C12").as_deref(), Some("8"));
        assert_eq!(seed_label(2020, "D", 4, "C1").as_deref(), Some("4"));
    }

    #[test]
    fn division_era_uses_division_letter_and_wildcard() {
        // 2014+: division seeds get the specific division letter.
        assert_eq!(seed_label(2024, "A", 1, "D1").as_deref(), Some("A1"));
        assert_eq!(seed_label(2024, "B", 2, "D2").as_deref(), Some("A2"));
        assert_eq!(seed_label(2024, "B", 3, "D3").as_deref(), Some("A3"));
        assert_eq!(seed_label(2024, "D", 2, "D2").as_deref(), Some("M2"));
        assert_eq!(seed_label(2024, "F", 3, "D3").as_deref(), Some("C3"));
        assert_eq!(seed_label(2024, "H", 2, "D2").as_deref(), Some("P2"));
        // Wildcards are shown verbatim regardless of series letter.
        assert_eq!(seed_label(2024, "A", 4, "WC1").as_deref(), Some("WC1"));
        assert_eq!(seed_label(2024, "C", 4, "WC2").as_deref(), Some("WC2"));
    }

    #[test]
    fn unsupported_years_have_no_label() {
        assert_eq!(seed_label(1993, "A", 1, "D1"), None);
        assert_eq!(seed_label(1980, "A", 1, ""), None);
    }
}
