use super::canvas::{TOP_MARGIN, RIGHT_PADDING, BOTTOM_PADDING};
use crate::conflict::Conflict;

#[derive(Clone)]
pub struct GraphDimensions {
    pub left_margin: f64,
    pub top_margin: f64,
    pub graph_width: f64,
    pub graph_height: f64,
    pub hour_width: f64,
}

impl GraphDimensions {
    #[must_use]
    pub fn new(canvas_width: f64, canvas_height: f64, station_label_width: f64) -> Self {
        let graph_width = canvas_width - station_label_width - RIGHT_PADDING;
        let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;
        let total_hours = 48.0; // Show 48 hours to support past-midnight

        Self {
            left_margin: station_label_width,
            top_margin: TOP_MARGIN,
            graph_width,
            graph_height,
            hour_width: graph_width / total_hours,
        }
    }
}

#[derive(Clone)]
pub struct ViewportState {
    pub zoom_level: f64,
    pub zoom_level_x: f64,
    pub pan_offset_x: f64,
    pub pan_offset_y: f64,
}

pub struct ConflictDisplayState<'a> {
    pub conflicts: &'a [Conflict],
    pub show_conflicts: bool,
}

pub struct HoverState<'a> {
    pub hovered_conflict: Option<&'a Conflict>,
    pub show_line_blocks: bool,
    pub hovered_journey_id: Option<&'a uuid::Uuid>,
}

/// Convert a hex color to rgba with the specified opacity
#[must_use]
pub fn hex_to_rgba(hex: &str, opacity: f64) -> String {
    let trimmed = hex.trim_start_matches('#');

    if trimmed.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&trimmed[0..2], 16),
            u8::from_str_radix(&trimmed[2..4], 16),
            u8::from_str_radix(&trimmed[4..6], 16),
        ) {
            return format!("rgba({r}, {g}, {b}, {opacity})");
        }
    }

    // Fallback: return original color if parsing fails
    hex.to_owned()
}