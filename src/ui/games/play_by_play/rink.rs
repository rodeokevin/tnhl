// Render a diagram of the rink and optionally a marker for a play-by-play event

use crate::ui::render::{ACCENT_BLUE, ACCENT_RED, BORDER_COLOR};
use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    symbols::Marker,
    widgets::canvas::{Canvas, Line, Points},
};

// NHL rink half-dimensions in feet
const RINK_HALF_X: f64 = 100.0;
const RINK_HALF_Y: f64 = 42.5;
// Fractional padding around the boards
const PAD_FRAC: f64 = 0.06;

// Full padded coordinate ranges (width along x, height along y).
const X_RANGE: f64 = RINK_HALF_X * 2.0 * (1.0 + PAD_FRAC);
const Y_RANGE: f64 = RINK_HALF_Y * 2.0 * (1.0 + PAD_FRAC);

// Braille sub-cell resolution
const DOTS_PER_COL: f64 = 2.0;
const DOTS_PER_ROW: f64 = 4.0;

const BLUE_LINE_X: f64 = 25.0;
const GOAL_LINE_X: f64 = 89.0;

// Radius of the rounded board corners, matching a standard NHL rink.
const CORNER_RADIUS: f64 = 28.0;

// End zone faceoff circles
const FACEOFF_X: f64 = GOAL_LINE_X - 20.0; // 20 ft in front of the goal line
const FACEOFF_Y: f64 = 22.0;
const FACEOFF_RADIUS: f64 = 15.0;
const FACEOFF_DOT_RADIUS: f64 = 1.0;

// Controls how finely curves are segmented
const ARC_STEPS_PER_UNIT_RADIUS: f64 = 2.5;
const MIN_CIRCLE_STEPS: f64 = 24.0;
const MAX_CIRCLE_STEPS: f64 = 180.0;

// The terminal's default text color, so the boards show on light and dark themes
const BOARDS_COLOR: Color = Color::Reset;
const EVENT_COLOR: Color = BORDER_COLOR;
// The event marker is a filled square; this is half its side length (ft).
const EVENT_HALF_SIZE: f64 = 1.0;

/// Given a width in columns, the number of rows needed so a braille canvas of
/// that size renders the rink
pub fn rows_for_width(cols: u16) -> u16 {
    let rows = cols as f64 * (DOTS_PER_COL * Y_RANGE) / (DOTS_PER_ROW * X_RANGE);
    rows.round().max(1.0) as u16
}

/// Render the rink diagram into `area`, plotting `coord` as a dot if present.
pub fn render_rink(frame: &mut Frame, area: Rect, coord: Option<(f64, f64)>) {
    let inner = fit_aspect(area);

    // Size of one braille dot in coordinate units
    let res_x = (inner.width as f64 * DOTS_PER_COL - 1.0).max(1.0);
    let res_y = (inner.height as f64 * DOTS_PER_ROW - 1.0).max(1.0);
    let dot_x = X_RANGE / res_x;
    let dot_y = Y_RANGE / res_y;

    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-X_RANGE / 2.0, X_RANGE / 2.0])
        .y_bounds([-Y_RANGE / 2.0, Y_RANGE / 2.0])
        .paint(move |ctx| {
            draw_rink(ctx);
            if let Some((x, y)) = coord {
                // Draw the marker on its own layer
                ctx.layer();
                // A small square as the event marker
                draw_filled_square(ctx, x, y, EVENT_HALF_SIZE, EVENT_COLOR, dot_x, dot_y);
            }
        });
    frame.render_widget(canvas, inner);
}

/// Compute the largest sub-rect of `area` that matches the rink's true aspect
/// ratio, centered within `area`.
fn fit_aspect(area: Rect) -> Rect {
    let mut w = area.width;
    let mut h = rows_for_width(w);
    if h > area.height {
        // Too tall so constrain by height instead
        h = area.height;
        // Invert rows_for_width: cols = rows · (4·X_RANGE) / (2·Y_RANGE)
        let cols = h as f64 * (DOTS_PER_ROW * X_RANGE) / (DOTS_PER_COL * Y_RANGE);
        w = (cols.round().max(1.0) as u16).min(area.width);
    }
    // Center the fitted rect within `area`
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// Draw the rink
fn draw_rink(ctx: &mut ratatui::widgets::canvas::Context) {
    draw_boards(ctx);

    // Center red line
    draw_vertical_ice_line(ctx, 0.0, ACCENT_RED);

    // Blue lines
    for x in [-BLUE_LINE_X, BLUE_LINE_X] {
        draw_vertical_ice_line(ctx, x, ACCENT_BLUE);
    }

    // Goal lines
    for x in [-GOAL_LINE_X, GOAL_LINE_X] {
        draw_vertical_ice_line(ctx, x, ACCENT_RED);
    }

    // Center faceoff circle
    draw_circle(ctx, 0.0, 0.0, FACEOFF_RADIUS, ACCENT_BLUE);

    // Four end-zone faceoff circles
    for &fx in &[-FACEOFF_X, FACEOFF_X] {
        for &fy in &[-FACEOFF_Y, FACEOFF_Y] {
            draw_circle(ctx, fx, fy, FACEOFF_RADIUS, ACCENT_RED);
            draw_circle(ctx, fx, fy, FACEOFF_DOT_RADIUS, ACCENT_RED);
        }
    }

    // Nets
    draw_nets(ctx);
}

/// Draw both goals just behind their goal lines
fn draw_nets(ctx: &mut ratatui::widgets::canvas::Context) {
    const NET_DEPTH: f64 = 4.0; // extent in x, behind the goal line
    const NET_HALF_WIDTH: f64 = 3.0; // half-extent in y (6 ft wide goal)

    for &goal_x in &[-GOAL_LINE_X, GOAL_LINE_X] {
        let back_x = if goal_x < 0.0 {
            goal_x - NET_DEPTH
        } else {
            goal_x + NET_DEPTH
        };
        // Three sides of the net
        ctx.draw(&Line::new(
            goal_x,
            -NET_HALF_WIDTH,
            back_x,
            -NET_HALF_WIDTH,
            ACCENT_RED,
        )); // post
        ctx.draw(&Line::new(
            goal_x,
            NET_HALF_WIDTH,
            back_x,
            NET_HALF_WIDTH,
            ACCENT_RED,
        )); // post
        ctx.draw(&Line::new(
            back_x,
            -NET_HALF_WIDTH,
            back_x,
            NET_HALF_WIDTH,
            ACCENT_RED,
        )); // back
    }
}

/// Draw a vertical line at `x` that reaches the boards
fn draw_vertical_ice_line(ctx: &mut ratatui::widgets::canvas::Context, x: f64, color: Color) {
    let half = board_half_width_at(x);
    ctx.draw(&Line::new(x, -half, x, half, color));
}

/// How far the boards are at a given x
fn board_half_width_at(x: f64) -> f64 {
    let straight_x = RINK_HALF_X - CORNER_RADIUS; // 72 ft
    let ax = x.abs();
    if ax <= straight_x {
        RINK_HALF_Y
    } else {
        // Corner arc: center at (straight_x, RINK_HALF_Y - CORNER_RADIUS),
        // radius CORNER_RADIUS. Solve for y on the arc at this x.
        let center_y = RINK_HALF_Y - CORNER_RADIUS;
        let dx = ax - straight_x;
        let inside = (CORNER_RADIUS * CORNER_RADIUS - dx * dx).max(0.0);
        center_y + inside.sqrt()
    }
}

fn draw_boards(ctx: &mut ratatui::widgets::canvas::Context) {
    let sx = RINK_HALF_X - CORNER_RADIUS; // straight-edge extent along x
    let sy = RINK_HALF_Y - CORNER_RADIUS; // straight-edge extent along y

    // Straight boards
    ctx.draw(&Line::new(
        -sx,
        -RINK_HALF_Y,
        sx,
        -RINK_HALF_Y,
        BOARDS_COLOR,
    )); // top
    ctx.draw(&Line::new(-sx, RINK_HALF_Y, sx, RINK_HALF_Y, BOARDS_COLOR)); // bottom
    ctx.draw(&Line::new(
        -RINK_HALF_X,
        -sy,
        -RINK_HALF_X,
        sy,
        BOARDS_COLOR,
    )); // left
    ctx.draw(&Line::new(RINK_HALF_X, -sy, RINK_HALF_X, sy, BOARDS_COLOR)); // right

    // Corner boards (rounded)
    draw_arc(ctx, -sx, -sy, CORNER_RADIUS, 180.0, 270.0, BOARDS_COLOR); // top-left
    draw_arc(ctx, sx, -sy, CORNER_RADIUS, 270.0, 360.0, BOARDS_COLOR); // top-right
    draw_arc(ctx, sx, sy, CORNER_RADIUS, 0.0, 90.0, BOARDS_COLOR); // bottom-right
    draw_arc(ctx, -sx, sy, CORNER_RADIUS, 90.0, 180.0, BOARDS_COLOR); // bottom-left
}

/// Draw a full circle centered at `(cx, cy)`
fn draw_circle(
    ctx: &mut ratatui::widgets::canvas::Context,
    cx: f64,
    cy: f64,
    radius: f64,
    color: Color,
) {
    draw_arc(ctx, cx, cy, radius, 0.0, 360.0, color);
}

/// Draw a filled square centered at `(cx, cy)` with the given `half_extent`
fn draw_filled_square(
    ctx: &mut ratatui::widgets::canvas::Context,
    cx: f64,
    cy: f64,
    half_extent: f64,
    color: Color,
    dot_x: f64,
    dot_y: f64,
) {
    let steps_x = (half_extent / dot_x).ceil() as i32;
    let steps_y = (half_extent / dot_y).ceil() as i32;

    let mut coords: Vec<(f64, f64)> = Vec::new();
    for iy in -steps_y..=steps_y {
        for ix in -steps_x..=steps_x {
            coords.push((cx + ix as f64 * dot_x, cy + iy as f64 * dot_y));
        }
    }
    if coords.is_empty() {
        coords.push((cx, cy));
    }
    ctx.draw(&Points {
        coords: &coords,
        color,
    });
}

/// Draw an arc centered at `(cx, cy)` from `start_deg` to `end_deg`
fn draw_arc(
    ctx: &mut ratatui::widgets::canvas::Context,
    cx: f64,
    cy: f64,
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    color: Color,
) {
    let sweep = end_deg - start_deg;
    let full_circle_steps = (radius * ARC_STEPS_PER_UNIT_RADIUS)
        .round()
        .clamp(MIN_CIRCLE_STEPS, MAX_CIRCLE_STEPS);
    let steps = ((full_circle_steps * sweep / 360.0).round() as usize).max(2);

    let mut prev: Option<(f64, f64)> = None;
    for i in 0..=steps {
        let angle = (start_deg + sweep * i as f64 / steps as f64).to_radians();
        let point = (cx + radius * angle.cos(), cy + radius * angle.sin());
        if let Some((px, py)) = prev {
            ctx.draw(&Line::new(px, py, point.0, point.1, color));
        }
        prev = Some(point);
    }
}
