use web_sys::CanvasRenderingContext2d;
use crate::models::TrainJourney;
use super::types::GraphDimensions;

// Train journey constants
const TRAIN_LINE_WIDTH: f64 = 2.0;
const STATION_DOT_RADIUS: f64 = 3.0;
const MIDNIGHT_WRAP_THRESHOLD: f64 = 0.5;

pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    train_journeys: &[TrainJourney],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
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