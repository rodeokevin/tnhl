use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Cell, Row, Table},
};

use crate::app::App;
use crate::models::games::games::PeriodType;
use crate::models::games::play_by_play::{
    DescKey, PlayData, PlayDetails, PlaysResponse, RosterPlayer, TypeDescKey,
};
use crate::ui::games::games::BIG_SCORE_COLOR;
use crate::ui::render::{ACCENT_RED, border_style};

const PLAYS_COLUMNS: [Constraint; 4] = [
    Constraint::Length(3),
    Constraint::Length(5),
    Constraint::Length(3),
    Constraint::Min(10),
];

/// Sum of the three fixed-width columns' widths.
const FIXED_COLUMNS_WIDTH: u16 = 3 + 5 + 3;
/// Spacing between 4 columns (3 gaps).
const COLUMN_SPACING: u16 = 1;
const NUM_GAPS: u16 = 3;

pub fn render_play_by_play(frame: &mut Frame, app: &mut App, area: Rect) {
    let block =
        Block::bordered()
            .title(" Play-by-Play ")
            .border_style(if app.state.games.plays_focused {
                border_style()
            } else {
                Style::new().fg(Color::DarkGray)
            });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // app.state.games.plays_visible_rows = inner.height.saturating_sub(1) as usize;

    let plays = app
        .state
        .games
        .games_data
        .as_ref()
        .and_then(|g| g.games.get(app.state.games.selected_game_index))
        .map(|g| g.id)
        .and_then(|id| app.state.games.plays_data.get(&id));

    let Some(plays) = plays else {
        // app.state.games.plays_len = 0;
        let missing = app.state.games.missing_text(
            &app.state.games.plays_unavailable,
            "Loading play-by-play...",
            "No play-by-play available for this game.",
        );
        frame.render_widget(Line::from(missing).centered(), inner);
        return;
    };

    // Resolve player IDs to roster entries for descriptions.
    let roster: HashMap<u32, &RosterPlayer> = plays
        .roster_spots
        .iter()
        .map(|r| (r.player_id, r))
        .collect();

    // Width available to the "Event" column once fixed columns + spacing are subtracted.
    let desc_width = event_column_width(inner.width);

    let rows: Vec<Row> = plays
        .plays
        .iter()
        .rev()
        .map(|p| play_row(p, plays, &roster, desc_width))
        .collect();
    // app.state.games.plays_len = rows.len();

    let table = Table::new(rows, PLAYS_COLUMNS)
        .header(
            Row::new(vec!["", "", "", "Event"])
                .style(Style::new().bold().add_modifier(Modifier::UNDERLINED)),
        )
        .column_spacing(COLUMN_SPACING)
        .row_highlight_style(Style::new().fg(Color::Gray).bg(Color::DarkGray).bold())
        .highlight_symbol("");
    frame.render_stateful_widget(table, inner, &mut app.state.games.plays_table_state);
}

/// Compute how wide the `Min(10)` "Event" column will actually render at,
/// given the total inner width, so we can wrap text to match.
fn event_column_width(total_width: u16) -> usize {
    let used = FIXED_COLUMNS_WIDTH + (COLUMN_SPACING * NUM_GAPS);
    total_width.saturating_sub(used).max(10) as usize
}

fn play_row(
    play: &PlayData,
    plays: &PlaysResponse,
    roster: &HashMap<u32, &RosterPlayer>,
    desc_width: usize,
) -> Row<'static> {
    let period = format_period(play, plays.game_type);

    let team = play
        .details
        .as_ref()
        .and_then(|d| d.event_owner_team_id)
        .map(|id| {
            if id == plays.home_team.id {
                plays.home_team.abbrev.clone()
            } else if id == plays.away_team.id {
                plays.away_team.abbrev.clone()
            } else {
                String::new()
            }
        })
        .unwrap_or_default();

    let desc = describe(play, roster);

    // Wrap the description to the column's actual rendered width, and size
    // the row to match how many lines that produced.
    let wrapped_lines = textwrap::wrap(&desc, desc_width.max(1));
    let height = wrapped_lines.len().max(1) as u16;
    let desc_text = Text::from(
        wrapped_lines
            .into_iter()
            .map(|l| Line::from(l.into_owned()))
            .collect::<Vec<_>>(),
    );

    Row::new(vec![
        Cell::from(period),
        Cell::from(play.time_remaining.clone()),
        Cell::from(team),
        Cell::from(desc_text),
    ])
    .height(height)
    .style(type_style(&play.type_desc_key))
}

/// Format the period column: `P1`/`P2`/`P3` in regulation, `OT`/`SO` otherwise.
fn format_period(play: &PlayData, game_type: u8) -> String {
    let Some(pd) = play.period_descriptor.as_ref() else {
        return "  ".to_string();
    };
    let is_playoffs = game_type == 3;
    match pd.period_type {
        PeriodType::REG => format!("P{}", pd.number),
        PeriodType::SO => "SO".to_string(),
        PeriodType::OT | PeriodType::Unknown => {
            // Regulation is 3 periods; OT number = period - 3.
            let ot_num = pd.number.saturating_sub(3);
            if is_playoffs && ot_num >= 2 {
                format!("OT{}", ot_num)
            } else {
                "OT".to_string()
            }
        }
    }
}

/// Resolve a player id to a last name + sweater number, or a placeholder when unknown.
fn name(roster: &HashMap<u32, &RosterPlayer>, id: Option<u32>) -> String {
    id.and_then(|id| roster.get(&id))
        .map(|p| p.short_name())
        .unwrap_or_else(|| "?".to_string())
}

/// Resolve a player id to `Name (#xx)`, or `None` if unknown/missing.
fn name_with_number(roster: &HashMap<u32, &RosterPlayer>, id: Option<u32>) -> Option<String> {
    id.and_then(|id| roster.get(&id))
        .map(|p| p.short_name().to_string())
}

/// Build an nhl.com-style description of a play from its details + roster.
fn describe(play: &PlayData, roster: &HashMap<u32, &RosterPlayer>) -> String {
    let details = play.details.as_ref();
    let empty = PlayDetails::default();
    let details = details.unwrap_or(&empty);
    let n = |id: Option<u32>| name(roster, id);

    match play.type_desc_key {
        TypeDescKey::Faceoff => format!(
            "Faceoff won by {} against {}",
            n(details.winning_player_id),
            n(details.losing_player_id)
        ),
        TypeDescKey::Hit => format!(
            "{} hit {}",
            n(details.hitting_player_id),
            n(details.hittee_player_id)
        ),
        TypeDescKey::ShotOnGoal => {
            let shot = details
                .shot_type
                .as_deref()
                .map(|s| format!("{} shot", s))
                .unwrap_or_else(|| "shot".to_string());
            format!(
                "{} {} saved by {}",
                n(details.shooting_player_id),
                shot,
                n(details.goalie_in_net_id)
            )
        }
        TypeDescKey::BlockedShot => format!(
            "{} shot blocked by {}",
            n(details.shooting_player_id),
            n(details.blocking_player_id)
        ),
        TypeDescKey::MissedShot => {
            let shot = details
                .shot_type
                .as_deref()
                .map(|s| format!("{} shot", s))
                .unwrap_or_else(|| "shot".to_string());
            format!("{} {} missed", n(details.shooting_player_id), shot)
        }
        TypeDescKey::Goal => {
            let shot = details.shot_type.as_deref().unwrap_or("");
            let mut s = format!("GOAL — {}", n(details.scoring_player_id));
            if !shot.is_empty() {
                s.push_str(&format!(" {} shot", shot));
            }
            let assists: Vec<String> = [details.assist1_player_id, details.assist2_player_id]
                .iter()
                .filter(|a| a.is_some())
                .map(|a| n(*a))
                .collect();
            if !assists.is_empty() {
                s.push_str(&format!(" assisted by {}", assists.join(", ")));
            }
            s
        }
        TypeDescKey::Penalty | TypeDescKey::Misconduct => format_penalty(details, roster),
        TypeDescKey::DelayedPenalty => "Delayed penalty".to_string(),
        TypeDescKey::Giveaway => format!("Giveaway by {}", n(details.player_id)),
        TypeDescKey::Takeaway => format!("Takeaway by {}", n(details.player_id)),
        TypeDescKey::Stoppage => {
            let reason = details
                .reason
                .as_deref()
                .unwrap_or("stoppage")
                .replace('-', " ");
            format!("Stoppage — {}", reason)
        }
        TypeDescKey::PeriodStart => "Period start".to_string(),
        TypeDescKey::PeriodEnd => "Period end".to_string(),
        TypeDescKey::ShootoutComplete => "Shootout complete".to_string(),
        TypeDescKey::GameEnd => "Game end".to_string(),
        TypeDescKey::Unknown => "—".to_string(),
    }
}

fn format_penalty(details: &PlayDetails, roster: &HashMap<u32, &RosterPlayer>) -> String {
    let Some(key) = details.desc_key.as_ref() else {
        log::debug!("No penalty type could be resolved");
        return "Penalty".to_string();
    };

    let duration = details
        .duration
        .map(|d| format!("{} min", d))
        .unwrap_or_default();

    // Standalone penalty-shot award (not tied to a specific infraction type).
    if matches!(key, DescKey::PenaltyShot | DescKey::PenaltyShotMinor) {
        let awarded_to = name_with_number(roster, details.drawn_by_player_id)
            .or_else(|| name_with_number(roster, details.committed_by_player_id))
            .unwrap_or_else(|| "?".to_string());
        return format!("Penalty shot awarded to {}", awarded_to);
    }

    // Infraction on a breakaway that results in a penalty shot.
    if is_penalty_shot_infraction(key) {
        let by = name_with_number(roster, details.committed_by_player_id)
            .unwrap_or_else(|| "?".to_string());
        let mut s = format!("Penalty shot for {} by {}", penalty_reason(key), by);
        if let Some(awarded_to) = name_with_number(roster, details.drawn_by_player_id) {
            s.push_str(&format!(" on {}", awarded_to));
        }
        return s;
    }

    if is_bench_penalty(key) {
        if let Some(server) = name_with_number(roster, details.served_by_player_id) {
            return format!(
                "{} for {} served by {}",
                duration,
                penalty_reason(key),
                server
            );
        } else {
            return format!("{} for {}", duration, penalty_reason(key));
        }
    }

    // Standard player minor/major/misconduct.
    let by =
        name_with_number(roster, details.committed_by_player_id).unwrap_or_else(|| "?".to_string());
    let mut s = format!("{} {} for {}", by, duration, penalty_reason(key));
    if let Some(drawn) = name_with_number(roster, details.drawn_by_player_id) {
        s.push_str(&format!(" {}", drawn));
    }
    // Include who serves the penalty when the API specifies a designated server.
    if let Some(server) = name_with_number(roster, details.served_by_player_id) {
        s.push_str(&format!(" served by {}", server));
    }
    s
}

fn is_bench_penalty(key: &DescKey) -> bool {
    matches!(
        key,
        DescKey::TooManyMenOnTheIce
            | DescKey::Bench
            | DescKey::DelayingGameBench
            | DescKey::DelayingGameBenchFaceOffViolation
            | DescKey::InterferenceBench
            | DescKey::UnsportsmanlikeConductBench
            | DescKey::IneligiblePlayer
            | DescKey::DelayingGameUnsuccessfulChallenge
            | DescKey::IllegalStickBench
    )
}

fn is_penalty_shot_infraction(key: &DescKey) -> bool {
    matches!(
        key,
        DescKey::PsHookingOnBreakaway
            | DescKey::PsSlashOnBreakaway
            | DescKey::PsHoldingOnBreakaway
            | DescKey::PsTrippingOnBreakaway
            | DescKey::PsThrowingObjectAtPuck
            | DescKey::PsCoveringPuckInCrease
            | DescKey::PsGoalkeeperDisplacedNet
            | DescKey::PsNetDisplaced
    )
}

/// The "for ___" phrase for every penalty type. This is the single place
/// each `DescKey` gets its own wording.
fn penalty_reason(key: &DescKey) -> &'static str {
    match key {
        // Minors
        DescKey::Boarding => "boarding",
        DescKey::Charging => "charging",
        DescKey::Clipping => "clipping",
        DescKey::CrossChecking => "cross-checking",
        DescKey::Diving => "diving",
        DescKey::Elbowing => "elbowing",
        DescKey::Embellishment => "embellishment",
        DescKey::Fighting => "fighting",
        DescKey::Holding => "holding",
        DescKey::HoldingTheStick => "holding the stick on",
        DescKey::Hooking => "hooking",
        DescKey::Interference => "interfering",
        DescKey::InterferenceGoalkeeper => "goalie interference on",
        DescKey::InterferenceBench => "bench interference",
        DescKey::Kneeing => "kneeing",
        DescKey::Roughing => "roughing",
        DescKey::RoughingRemovingOpponentsHelmet => "roughing (removing opponent's helmet)",
        DescKey::Slashing => "slashing",
        DescKey::Tripping => "tripping",
        DescKey::HighSticking => "high-sticking",
        DescKey::Instigator => "instigating",
        DescKey::InstigatorMisconduct => "instigating (misconduct)",
        DescKey::InstigatorFaceShield => "instigating (face shield)",
        DescKey::UnsportsmanlikeConduct => "unsportsmanlike conduct",
        DescKey::UnsportsmanlikeConductBench => "unsportsmanlike conduct",
        DescKey::Spearing => "spearing",
        DescKey::CheckingFromBehind => "checking from behind",
        DescKey::IllegalCheckToHead => "an illegal check to the head",
        DescKey::ClosingHandOnPuck => "closing hand on the puck",
        DescKey::BrokenStick => "playing with a broken stick",

        // Double minors
        DescKey::HighStickingDoubleMinor => "high-sticking (double minor)",
        DescKey::RoughingDoubleMinor => "roughing (double minor)",
        DescKey::SpearingDoubleMinor => "spearing (double minor)",
        DescKey::ButtEndingDoubleMinor => "butt-ending (double minor)",

        // Delay of game
        DescKey::DelayingGame => "delay of game",
        DescKey::DelayingGameBench => "delay of game",
        DescKey::DelayingGamePuckOverGlass => "delay of game (puck over glass)",
        DescKey::DelayingGameUnsuccessfulChallenge => "unsuccessful coach's challenge",
        DescKey::DelayingGameSmotheringPuck => "delay of game (smothering the puck)",
        DescKey::DelayingGameFaceOffViolation => "faceoff violation",
        DescKey::DelayingGameBenchFaceOffViolation => "faceoff violation (bench)",
        DescKey::DelayingGameIllegalPlayByGoalie => "an illegal play by the goalie",
        DescKey::DelayingGameEquipment => "delay of game (equipment)",

        // Bench / team
        DescKey::Bench => "a bench penalty",
        DescKey::TooManyMenOnTheIce => "too many men on the ice",
        DescKey::IllegalSubstitution => "illegal substitution",
        DescKey::IllegalStickBench => "illegal stick (bench)",
        DescKey::UnsportsmanlikeConductCoach => "unsportsmanlike conduct (coach)",

        // Majors / misconduct
        DescKey::AttemptToInjure => "attempt to injure",
        DescKey::HeadButting => "headbutting",
        DescKey::Misconduct => "misconduct",
        DescKey::GameMisconduct => "game misconduct",
        DescKey::GrossMisconduct => "gross misconduct",
        DescKey::GameMisconductHeadCoach => "game misconduct (head coach)",
        DescKey::MatchPenalty => "match penalty",
        DescKey::MatchPenatly10Minutes => "match penalty",

        // Goalie / equipment
        DescKey::GoalieLeaveCrease => "leaving the crease",
        DescKey::GoalieRemovedOwnMask => "removing his own mask",
        DescKey::GoalieParticipationBeyondCenter => "playing the puck beyond center ice",
        DescKey::PuckThrownForwardGoalkeeper => "throwing the puck forward",
        DescKey::ThrowingEquipment => "throwing equipment",
        DescKey::IllegalEquipment => "illegal equipment",
        DescKey::IllegalStick => "illegal stick",
        DescKey::PlayingWithoutAHelmet => "playing without a helmet",

        // Penalty shot infractions (used within is_penalty_shot_infraction path)
        DescKey::PsHookingOnBreakaway => "hooking on a breakaway",
        DescKey::PsSlashOnBreakaway => "slashing on a breakaway",
        DescKey::PsHoldingOnBreakaway => "holding on a breakaway",
        DescKey::PsTrippingOnBreakaway => "tripping on a breakaway",
        DescKey::PsThrowingObjectAtPuck => "throwing an object at the puck",
        DescKey::PsCoveringPuckInCrease => "covering the puck in the crease",
        DescKey::PsGoalkeeperDisplacedNet => "displacing the net",
        DescKey::PsNetDisplaced => "the net being displaced",
        DescKey::PenaltyShot | DescKey::PenaltyShotMinor => "a penalty shot",

        // Other
        DescKey::AbuseOfOfficials => "abuse of officials",
        DescKey::AbusiveLanguage => "abusive language",
        DescKey::Aggressor => "being the aggressor",
        DescKey::IneligiblePlayer => "an ineligible player",
        DescKey::InterferenceWithOfficial => "interference with official",
        DescKey::Minor => "a minor penalty",
        DescKey::Unknown => "a penalty",
    }
}

fn type_style(kind: &TypeDescKey) -> Style {
    match kind {
        TypeDescKey::Goal => Style::new().fg(BIG_SCORE_COLOR).bold(),
        TypeDescKey::Penalty | TypeDescKey::Misconduct | TypeDescKey::DelayedPenalty => {
            Style::new().fg(ACCENT_RED)
        }
        TypeDescKey::Stoppage
        | TypeDescKey::PeriodStart
        | TypeDescKey::PeriodEnd
        | TypeDescKey::ShootoutComplete
        | TypeDescKey::GameEnd
        | TypeDescKey::Unknown => Style::new().fg(Color::DarkGray),
        _ => Style::default(),
    }
}
