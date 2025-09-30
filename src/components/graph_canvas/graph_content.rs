use web_sys::CanvasRenderingContext2d;
use chrono::NaiveDateTime;
use crate::models::{SegmentState, TrainJourney};
use crate::constants::BASE_DATE;
use super::types::GraphDimensions;

// Background constants
const BACKGROUND_COLOR: &str = "#0a0a0a";

// Station grid constants
const STATION_GRID_COLOR: &str = "#1a1a1a";
const GRID_PADDING_HOURS: i32 = 5;

// Double track indicator constants
const DOUBLE_TRACK_BG_COLOR: &str = "rgba(255, 255, 255, 0.03)";

// Train journey constants
const TRAIN_LINE_WIDTH: f64 = 2.0;
const STATION_DOT_RADIUS: f64 = 3.0;
const MIDNIGHT_WRAP_THRESHOLD: f64 = 0.5;

// Current train position constants
const CURRENT_TRAIN_RADIUS: f64 = 6.0;
const CURRENT_TRAIN_OUTLINE_COLOR: &str = "#fff";
const CURRENT_TRAIN_OUTLINE_WIDTH: f64 = 2.0;
const CURRENT_TRAIN_LABEL_COLOR: &str = "#fff";
const CURRENT_TRAIN_LABEL_FONT_SIZE: f64 = 10.0;

// Conflict highlight constants
const CONFLICT_TRIANGLE_SIZE: f64 = 15.0;
const CONFLICT_LINE_WIDTH: f64 = 1.5;
const CONFLICT_FILL_COLOR: &str = "rgba(255, 200, 0, 0.9)";
const CONFLICT_STROKE_COLOR: &str = "rgba(0, 0, 0, 0.8)";
const CONFLICT_ICON_COLOR: &str = "#000";
const CONFLICT_ICON_FONT_SIZE: f64 = 12.0;
const CONFLICT_LABEL_COLOR: &str = "rgba(255, 255, 255, 0.9)";
const CONFLICT_LABEL_FONT_SIZE: f64 = 9.0;
const CONFLICT_WARNING_COLOR: &str = "rgba(255, 0, 0, 0.8)";
const CONFLICT_WARNING_FONT_SIZE: f64 = 14.0;
const MAX_CONFLICTS_DISPLAYED: usize = 1000;

// Time indicator constants
const TIME_INDICATOR_BG_COLOR: &str = "rgba(255, 51, 51, 0.3)";
const TIME_INDICATOR_BG_WIDTH: f64 = 8.0;
const TIME_INDICATOR_LINE_COLOR: &str = "#FF3333";
const TIME_INDICATOR_LINE_WIDTH: f64 = 2.0;
const TIME_INDICATOR_HANDLE_SIZE: f64 = 8.0;
const TIME_INDICATOR_LABEL_FONT: &str = "bold 12px monospace";

pub fn draw_background(ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
    ctx.set_fill_style_str(BACKGROUND_COLOR);
    ctx.fill_rect(0.0, 0.0, width, height);
}

pub fn draw_station_grid(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, stations: &[String]) {
    let station_height = dims.graph_height / stations.len() as f64;

    for (i, _station) in stations.iter().enumerate() {
        let y = calculate_station_y(dims, i, station_height);
        draw_horizontal_line(ctx, dims, y);
    }
}

fn calculate_station_y(dims: &GraphDimensions, index: usize, station_height: f64) -> f64 {
    dims.top_margin + (index as f64 * station_height) + (station_height / 2.0)
}

fn draw_horizontal_line(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, y: f64) {
    ctx.set_stroke_style_str(STATION_GRID_COLOR);
    ctx.begin_path();

    // Calculate the same extended range as the hour grid
    let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
    let start_hour = -GRID_PADDING_HOURS;
    let end_hour = hours_visible + GRID_PADDING_HOURS;

    let start_x = dims.left_margin + (start_hour as f64 * dims.hour_width);
    let end_x = dims.left_margin + (end_hour as f64 * dims.hour_width);

    ctx.move_to(start_x, y);
    ctx.line_to(end_x, y);
    ctx.stroke();
}

pub fn draw_double_track_indicators(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    segment_state: &SegmentState,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    // Draw lighter background for double-tracked segments
    for &segment_idx in &segment_state.double_tracked_segments {
        if segment_idx > 0 && segment_idx < stations.len() {
            // Calculate the Y positions for the two stations
            let station1_y = calculate_station_y(dims, segment_idx - 1, station_height);
            let station2_y = calculate_station_y(dims, segment_idx, station_height);

            // Cover the entire area between the two stations
            let top_y = station1_y.min(station2_y);
            let height = (station2_y - station1_y).abs();

            // Calculate the same extended range as other grid elements
            let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
            let start_hour = -GRID_PADDING_HOURS;
            let end_hour = hours_visible + GRID_PADDING_HOURS;
            let start_x = dims.left_margin + (start_hour as f64 * dims.hour_width);
            let width = (end_hour - start_hour) as f64 * dims.hour_width;

            // Draw lighter background rectangle
            ctx.set_fill_style_str(DOUBLE_TRACK_BG_COLOR);
            ctx.fill_rect(start_x, top_y, width, height);
        }
    }
}

pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    train_journeys: &[TrainJourney],
    zoom_level: f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    for journey in train_journeys {
        ctx.set_stroke_style_str(&journey.color);
        ctx.set_line_width(TRAIN_LINE_WIDTH / zoom_level);
        ctx.begin_path();

        let mut first_point = true;
        let mut prev_x = 0.0;

        for (station_name, arrival_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s == station_name) {
                let time_fraction = time_to_fraction(*arrival_time);
                let mut x = dims.left_margin + (time_fraction * dims.hour_width);

                // If this x position is much less than the previous x (indicating midnight wrap),
                // add the width of one full day to continue the line
                if !first_point && x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                    x += dims.graph_width;
                }
                let y = dims.top_margin
                    + (station_idx as f64 * station_height)
                    + (station_height / 2.0);

                if first_point {
                    ctx.move_to(x, y);
                    first_point = false;
                } else {
                    ctx.line_to(x, y);
                }

                prev_x = x;
            }
        }

        ctx.stroke();

        // Draw small dots at each station stop
        let mut prev_x = 0.0;
        for (station_name, arrival_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s == station_name) {
                let time_fraction = time_to_fraction(*arrival_time);
                let mut x = dims.left_margin + (time_fraction * dims.hour_width);

                // Handle midnight wrap-around for station dots
                if prev_x > 0.0 && x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                    x += dims.graph_width;
                }

                let y = dims.top_margin
                    + (station_idx as f64 * station_height)
                    + (station_height / 2.0);

                ctx.set_fill_style_str(&journey.color);
                ctx.begin_path();
                let _ = ctx.arc(x, y, STATION_DOT_RADIUS / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                ctx.fill();

                prev_x = x;
            }
        }
    }
}

pub fn draw_current_train_positions(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    train_journeys: &[TrainJourney],
    station_height: f64,
    visualization_time: NaiveDateTime,
    zoom_level: f64,
) {
    for journey in train_journeys {
        // Find which segment the train is currently on
        let mut prev_station: Option<(&String, NaiveDateTime, usize)> = None;
        let mut next_station: Option<(&String, NaiveDateTime, usize)> = None;

        for (station_name, arrival_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s == station_name) {
                if *arrival_time <= visualization_time {
                    prev_station = Some((station_name, *arrival_time, station_idx));
                } else if next_station.is_none() {
                    next_station = Some((station_name, *arrival_time, station_idx));
                    break;
                }
            }
        }

        // If train is between two stations, interpolate its position
        if let (Some((_, prev_time, prev_idx)), Some((_, next_time, next_idx))) =
            (prev_station, next_station)
        {
            let segment_duration = next_time.signed_duration_since(prev_time).num_seconds() as f64;
            let elapsed = visualization_time
                .signed_duration_since(prev_time)
                .num_seconds() as f64;
            let progress = (elapsed / segment_duration).clamp(0.0, 1.0);

            let prev_x = dims.left_margin + (time_to_fraction(prev_time) * dims.hour_width);
            let prev_y =
                dims.top_margin + (prev_idx as f64 * station_height) + (station_height / 2.0);

            let next_x = dims.left_margin + (time_to_fraction(next_time) * dims.hour_width);
            let next_y =
                dims.top_margin + (next_idx as f64 * station_height) + (station_height / 2.0);

            let current_x = prev_x + (next_x - prev_x) * progress;
            let current_y = prev_y + (next_y - prev_y) * progress;

            // Draw train as a larger dot with an outline
            ctx.set_fill_style_str(&journey.color);
            ctx.set_stroke_style_str(CURRENT_TRAIN_OUTLINE_COLOR);
            ctx.set_line_width(CURRENT_TRAIN_OUTLINE_WIDTH / zoom_level);
            ctx.begin_path();
            let _ = ctx.arc(
                current_x,
                current_y,
                CURRENT_TRAIN_RADIUS / zoom_level,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            ctx.fill();
            ctx.stroke();

            // Draw train ID label with zoom-compensated font size
            ctx.set_fill_style_str(CURRENT_TRAIN_LABEL_COLOR);
            ctx.set_font(&format!("bold {}px monospace", CURRENT_TRAIN_LABEL_FONT_SIZE / zoom_level));
            let _ = ctx.fill_text(
                &journey.line_id,
                current_x - 12.0 / zoom_level,
                current_y - 10.0 / zoom_level,
            );
        }
    }
}

pub fn draw_conflict_highlights(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflicts: &[crate::models::Conflict],
    station_height: f64,
    zoom_level: f64,
) {
    // Limit to first 1000 conflicts to prevent performance issues
    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        // Calculate y position based on the conflict position between stations
        let y = dims.top_margin
            + (conflict.station1_idx as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (conflict.station2_idx - conflict.station1_idx) as f64);

        // Draw a warning triangle at the conflict point
        let size = CONFLICT_TRIANGLE_SIZE / zoom_level;
        ctx.set_line_width(CONFLICT_LINE_WIDTH / zoom_level);

        // Draw filled triangle
        ctx.begin_path();
        ctx.move_to(x, y - size); // Top point
        ctx.line_to(x - size * 0.866, y + size * 0.5); // Bottom left
        ctx.line_to(x + size * 0.866, y + size * 0.5); // Bottom right
        ctx.close_path();

        // Fill with warning color
        ctx.set_fill_style_str(CONFLICT_FILL_COLOR);
        ctx.fill();

        // Stroke with thick black border
        ctx.set_stroke_style_str(CONFLICT_STROKE_COLOR);
        ctx.stroke();

        // Draw exclamation mark inside triangle
        ctx.set_fill_style_str(CONFLICT_ICON_COLOR);
        ctx.set_font(&format!("bold {}px sans-serif", CONFLICT_ICON_FONT_SIZE / zoom_level));
        let _ = ctx.fill_text("!", x - 2.0 / zoom_level, y + 4.0 / zoom_level);

        // Draw conflict details (simplified - just show line IDs)
        ctx.set_fill_style_str(CONFLICT_LABEL_COLOR);
        ctx.set_font(&format!("{}px monospace", CONFLICT_LABEL_FONT_SIZE / zoom_level));
        let label = format!("{} × {}", conflict.journey1_id, conflict.journey2_id);
        let _ = ctx.fill_text(&label, x + size + 5.0 / zoom_level, y);
    }

    // If there are more conflicts than displayed, show a count
    if conflicts.len() > MAX_CONFLICTS_DISPLAYED {
        ctx.set_fill_style_str(CONFLICT_WARNING_COLOR);
        ctx.set_font(&format!("bold {}px monospace", CONFLICT_WARNING_FONT_SIZE / zoom_level));
        let warning_text = format!(
            "⚠ {} more conflicts not shown",
            conflicts.len() - MAX_CONFLICTS_DISPLAYED
        );
        let _ = ctx.fill_text(&warning_text, 10.0, dims.top_margin - 10.0);
    }
}

pub fn draw_time_indicator(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    time: NaiveDateTime,
    zoom_level: f64,
    pan_offset_x: f64,
) {
    let time_fraction = time_to_fraction(time);
    let base_x = time_fraction * dims.hour_width;
    let x = dims.left_margin + (base_x * zoom_level) + pan_offset_x;

    // Only draw if the time indicator is within the visible graph area
    if x < dims.left_margin || x > dims.left_margin + dims.graph_width {
        return;
    }

    // Draw semi-transparent background for the line
    ctx.set_stroke_style_str(TIME_INDICATOR_BG_COLOR);
    ctx.set_line_width(TIME_INDICATOR_BG_WIDTH);
    ctx.begin_path();
    ctx.move_to(x, dims.top_margin);
    ctx.line_to(x, dims.top_margin + dims.graph_height);
    ctx.stroke();

    // Draw main line
    ctx.set_stroke_style_str(TIME_INDICATOR_LINE_COLOR);
    ctx.set_line_width(TIME_INDICATOR_LINE_WIDTH);
    ctx.begin_path();
    ctx.move_to(x, dims.top_margin);
    ctx.line_to(x, dims.top_margin + dims.graph_height);
    ctx.stroke();

    // Draw draggable handle at top
    ctx.set_fill_style_str(TIME_INDICATOR_LINE_COLOR);
    ctx.begin_path();
    ctx.move_to(x - TIME_INDICATOR_HANDLE_SIZE, dims.top_margin - 15.0);
    ctx.line_to(x + TIME_INDICATOR_HANDLE_SIZE, dims.top_margin - 15.0);
    ctx.line_to(x, dims.top_margin - 5.0);
    ctx.close_path();
    ctx.fill();

    // Draw time label
    ctx.set_fill_style_str(TIME_INDICATOR_LINE_COLOR);
    ctx.set_font(TIME_INDICATOR_LABEL_FONT);
    let _ = ctx.fill_text(
        &time.format("%H:%M").to_string(),
        x - 20.0,
        dims.top_margin - 20.0,
    );
}

pub fn time_to_fraction(time: chrono::NaiveDateTime) -> f64 {
    // Calculate hours from the base date (2024-01-01 00:00:00)
    let base_datetime = BASE_DATE.and_hms_opt(0, 0, 0).expect("Valid datetime");

    let duration_since_base = time.signed_duration_since(base_datetime);
    let total_seconds = duration_since_base.num_seconds() as f64;
    total_seconds / 3600.0 // Convert to hours
}