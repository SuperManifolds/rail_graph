use crate::models::Station;
use crate::components::graph_canvas::{TOP_MARGIN, BOTTOM_PADDING};

// Toggle button constants
const TOGGLE_X: f64 = 85.0;
const TOGGLE_SIZE: f64 = 12.0;

/// Check if a mouse click hit a toggle button for double-track segments
pub fn check_toggle_click(
    mouse_x: f64,
    mouse_y: f64,
    canvas_height: f64,
    stations: &[Station],
    zoom_level: f64,
    pan_offset_y: f64,
) -> Option<usize> {
    let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;

    let station_height = graph_height / stations.len() as f64;

    // Check if click is in the toggle area horizontally
    if (TOGGLE_X - TOGGLE_SIZE/2.0..=TOGGLE_X + TOGGLE_SIZE/2.0).contains(&mouse_x) {
        // Check each segment toggle
        for i in 1..stations.len() {
            let segment_index = i;

            // Calculate position between the two stations (same logic as draw_segment_toggles)
            let base_y1 = ((i - 1) as f64 * station_height) + (station_height / 2.0);
            let base_y2 = (i as f64 * station_height) + (station_height / 2.0);
            let center_y = (base_y1 + base_y2) / 2.0;
            let adjusted_y = TOP_MARGIN + (center_y * zoom_level) + pan_offset_y;

            // Check if click is within this toggle button
            if (adjusted_y - TOGGLE_SIZE/2.0..=adjusted_y + TOGGLE_SIZE/2.0).contains(&mouse_y) {
                return Some(segment_index);
            }
        }
    }

    None
}