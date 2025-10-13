use web_sys::CanvasRenderingContext2d;
use chrono::NaiveDateTime;
use crate::models::StationNode;
use crate::train_journey::TrainJourney;
use super::types::GraphDimensions;
use petgraph::stable_graph::NodeIndex;
use std::collections::HashMap;

// Current train position constants
const CURRENT_TRAIN_RADIUS: f64 = 6.0;
const CURRENT_TRAIN_OUTLINE_COLOR: &str = "#fff";
const CURRENT_TRAIN_OUTLINE_WIDTH: f64 = 2.0;
const CURRENT_TRAIN_LABEL_COLOR: &str = "#fff";
const CURRENT_TRAIN_LABEL_FONT_SIZE: f64 = 10.0;

#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
pub fn draw_current_train_positions(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[(NodeIndex, StationNode)],
    train_journeys: &[&TrainJourney],
    station_height: f64,
    visualization_time: NaiveDateTime,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    // Build NodeIndex to display position map
    let station_positions: HashMap<NodeIndex, usize> = stations
        .iter()
        .enumerate()
        .map(|(idx, (node_idx, _))| (*node_idx, idx))
        .collect();

    for journey in train_journeys {
        // Find which segment the train is currently on (or if it's waiting at a station)
        let mut prev_departure: Option<(NodeIndex, NaiveDateTime, usize)> = None;
        let mut next_arrival: Option<(NodeIndex, NaiveDateTime, usize)> = None;

        for (node_idx, arrival_time, departure_time) in &journey.station_times {
            if let Some(&station_idx) = station_positions.get(node_idx) {
                // Check if train is currently waiting at this station
                if *arrival_time <= visualization_time && visualization_time <= *departure_time {
                    // Train is waiting at this station
                    let x = dims.left_margin + (time_to_fraction(visualization_time) * dims.hour_width);
                    let y = dims.top_margin + (station_idx as f64 * station_height) + (station_height / 2.0);

                    // Draw train as a larger dot with an outline
                    ctx.set_fill_style_str(&journey.color);
                    ctx.set_stroke_style_str(CURRENT_TRAIN_OUTLINE_COLOR);
                    ctx.set_line_width(CURRENT_TRAIN_OUTLINE_WIDTH / zoom_level);
                    ctx.begin_path();
                    let _ = ctx.arc(x, y, CURRENT_TRAIN_RADIUS / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                    ctx.fill();
                    ctx.stroke();

                    // Draw train ID label
                    ctx.set_fill_style_str(CURRENT_TRAIN_LABEL_COLOR);
                    ctx.set_font(&format!("bold {}px monospace", CURRENT_TRAIN_LABEL_FONT_SIZE / zoom_level));
                    let _ = ctx.fill_text(&journey.line_id, x - 12.0 / zoom_level, y - 10.0 / zoom_level);
                    break;
                }

                if *departure_time <= visualization_time {
                    prev_departure = Some((*node_idx, *departure_time, station_idx));
                } else if next_arrival.is_none() {
                    next_arrival = Some((*node_idx, *arrival_time, station_idx));
                    break;
                }
            }
        }

        // If train is traveling between two stations, interpolate its position
        if let (Some((_, prev_time, prev_idx)), Some((_, next_time, next_idx))) =
            (prev_departure, next_arrival)
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