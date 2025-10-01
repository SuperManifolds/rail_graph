use super::canvas::{LEFT_MARGIN, TOP_MARGIN, RIGHT_PADDING, BOTTOM_PADDING};

#[derive(Clone)]
pub struct GraphDimensions {
    pub left_margin: f64,
    pub top_margin: f64,
    pub graph_width: f64,
    pub graph_height: f64,
    pub hour_width: f64,
}

impl GraphDimensions {
    pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
        let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;
        let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;
        let total_hours = 48.0; // Show 48 hours to support past-midnight

        Self {
            left_margin: LEFT_MARGIN,
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