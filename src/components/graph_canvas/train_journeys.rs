use web_sys::CanvasRenderingContext2d;
use crate::models::StationNode;
use crate::train_journey::TrainJourney;
use super::types::GraphDimensions;

// Train journey constants
const MIDNIGHT_WRAP_THRESHOLD: f64 = 0.5;

pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[StationNode],
    train_journeys: &[TrainJourney],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    for journey in train_journeys {
        ctx.set_stroke_style_str(&journey.color);
        ctx.set_line_width(journey.thickness / zoom_level);
        ctx.begin_path();

        let mut first_point = true;
        let mut prev_x = 0.0;

        for (station_name, arrival_time, departure_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s.name == *station_name) {
                let arrival_fraction = time_to_fraction(*arrival_time);
                let departure_fraction = time_to_fraction(*departure_time);
                let mut arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
                let mut departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

                // If this x position is much less than the previous x (indicating midnight wrap),
                // add the width of one full day to continue the line
                if !first_point && arrival_x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                    arrival_x += dims.graph_width;
                    departure_x += dims.graph_width;
                }
                let y = dims.top_margin
                    + (station_idx as f64 * station_height)
                    + (station_height / 2.0);

                if first_point {
                    ctx.move_to(arrival_x, y);
                    first_point = false;
                } else {
                    // Draw diagonal line to arrival point
                    ctx.line_to(arrival_x, y);
                }

                // Draw vertical line for wait time (from arrival to departure)
                if arrival_x != departure_x {
                    ctx.line_to(departure_x, y);
                }

                prev_x = departure_x;
            }
        }

        ctx.stroke();

        // Draw small dots at arrival and departure points (scale with line thickness)
        let dot_radius = (journey.thickness * 1.5).max(2.0); // Scale dots with thickness, minimum 2.0
        let mut prev_x = 0.0;
        for (station_name, arrival_time, departure_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s.name == *station_name) {
                let arrival_fraction = time_to_fraction(*arrival_time);
                let departure_fraction = time_to_fraction(*departure_time);
                let mut arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
                let mut departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

                // Handle midnight wrap-around for station dots
                if prev_x > 0.0 && arrival_x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                    arrival_x += dims.graph_width;
                    departure_x += dims.graph_width;
                }

                let y = dims.top_margin
                    + (station_idx as f64 * station_height)
                    + (station_height / 2.0);

                ctx.set_fill_style_str(&journey.color);

                // Draw dot at arrival point
                ctx.begin_path();
                let _ = ctx.arc(arrival_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                ctx.fill();

                // Draw dot at departure point (if different from arrival)
                if arrival_x != departure_x {
                    ctx.begin_path();
                    let _ = ctx.arc(departure_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                    ctx.fill();
                }

                prev_x = departure_x;
            }
        }
    }
}