use web_sys::CanvasRenderingContext2d;
use chrono::NaiveDateTime;
use crate::models::Node;
use crate::theme::Theme;
use crate::train_journey::TrainJourney;
use super::types::GraphDimensions;
use petgraph::stable_graph::NodeIndex;

const CURRENT_TRAIN_RADIUS: f64 = 6.0;
const CURRENT_TRAIN_OUTLINE_WIDTH: f64 = 2.0;
const CURRENT_TRAIN_LABEL_FONT_SIZE: f64 = 10.0;

struct Palette {
    train_outline: &'static str,
    train_label: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    train_outline: "#fff",
    train_label: "#fff",
};

const LIGHT_PALETTE: Palette = Palette {
    train_outline: "#000",
    train_label: "#000",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
pub fn draw_current_train_positions(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[(NodeIndex, Node)],
    train_journeys: &[&TrainJourney],
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    visualization_time: NaiveDateTime,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    theme: Theme,
) {
    let palette = get_palette(theme);

    for journey in train_journeys {
        // Match journey stations to view positions using edge-based matching
        let station_positions = super::train_journeys::match_journey_stations_to_view_by_edges(
            &journey.segments,
            &journey.station_times,
            view_edge_path,
            stations,
        );

        // Find which segment the train is currently on (or if it's waiting at a station)
        let mut prev_departure: Option<(usize, NaiveDateTime, usize)> = None;
        let mut next_arrival: Option<(usize, NaiveDateTime, usize)> = None;

        for (i, (_node_idx, arrival_time, departure_time)) in journey.station_times.iter().enumerate() {
            if let Some(station_idx) = station_positions.get(i).and_then(|&opt| opt) {
                // Check if train is currently waiting at this station
                if *arrival_time <= visualization_time && visualization_time <= *departure_time {
                    // Train is waiting at this station
                    let x = dims.left_margin + (time_to_fraction(visualization_time) * dims.hour_width);
                    // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
                    let y = station_y_positions[station_idx] - super::canvas::TOP_MARGIN;

                    // Draw train as a larger dot with an outline
                    ctx.set_fill_style_str(&journey.color);
                    ctx.set_stroke_style_str(palette.train_outline);
                    ctx.set_line_width(CURRENT_TRAIN_OUTLINE_WIDTH / zoom_level);
                    ctx.begin_path();
                    let _ = ctx.arc(x, y, CURRENT_TRAIN_RADIUS / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                    ctx.fill();
                    ctx.stroke();

                    // Draw train number label
                    ctx.set_fill_style_str(palette.train_label);
                    ctx.set_font(&format!("bold {}px monospace", CURRENT_TRAIN_LABEL_FONT_SIZE / zoom_level));
                    let _ = ctx.fill_text(&journey.train_number, x - 12.0 / zoom_level, y - 10.0 / zoom_level);
                    break;
                }

                if *departure_time <= visualization_time {
                    prev_departure = Some((i, *departure_time, station_idx));
                } else if next_arrival.is_none() {
                    next_arrival = Some((i, *arrival_time, station_idx));
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
            // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
            let prev_y = station_y_positions[prev_idx] - super::canvas::TOP_MARGIN;

            let next_x = dims.left_margin + (time_to_fraction(next_time) * dims.hour_width);
            let next_y = station_y_positions[next_idx] - super::canvas::TOP_MARGIN;

            let current_x = prev_x + (next_x - prev_x) * progress;
            let current_y = prev_y + (next_y - prev_y) * progress;

            // Draw train as a larger dot with an outline
            ctx.set_fill_style_str(&journey.color);
            ctx.set_stroke_style_str(palette.train_outline);
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

            // Draw train number label with zoom-compensated font size
            ctx.set_fill_style_str(palette.train_label);
            ctx.set_font(&format!("bold {}px monospace", CURRENT_TRAIN_LABEL_FONT_SIZE / zoom_level));
            let _ = ctx.fill_text(
                &journey.train_number,
                current_x - 12.0 / zoom_level,
                current_y - 10.0 / zoom_level,
            );
        }
    }
}