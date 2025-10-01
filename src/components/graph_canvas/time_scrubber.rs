use web_sys::CanvasRenderingContext2d;
use chrono::NaiveDateTime;
use super::types::GraphDimensions;

// Time scrubber constants
const TIME_SCRUBBER_BG_COLOR: &str = "rgba(255, 51, 51, 0.3)";
const TIME_SCRUBBER_BG_WIDTH: f64 = 8.0;
const TIME_SCRUBBER_LINE_COLOR: &str = "#FF3333";
const TIME_SCRUBBER_LINE_WIDTH: f64 = 2.0;
const TIME_SCRUBBER_HANDLE_SIZE: f64 = 8.0;
const TIME_SCRUBBER_LABEL_FONT: &str = "bold 12px monospace";

pub fn draw_time_scrubber(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    time: NaiveDateTime,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    time_to_fraction: fn(NaiveDateTime) -> f64,
) {
    let time_fraction = time_to_fraction(time);
    let base_x = time_fraction * dims.hour_width;
    let x = dims.left_margin + (base_x * zoom_level * zoom_level_x) + pan_offset_x;

    // Only draw if the time scrubber is within the visible graph area
    if x < dims.left_margin || x > dims.left_margin + dims.graph_width {
        return;
    }

    // Draw semi-transparent background for the line
    ctx.set_stroke_style_str(TIME_SCRUBBER_BG_COLOR);
    ctx.set_line_width(TIME_SCRUBBER_BG_WIDTH);
    ctx.begin_path();
    ctx.move_to(x, dims.top_margin);
    ctx.line_to(x, dims.top_margin + dims.graph_height);
    ctx.stroke();

    // Draw main line
    ctx.set_stroke_style_str(TIME_SCRUBBER_LINE_COLOR);
    ctx.set_line_width(TIME_SCRUBBER_LINE_WIDTH);
    ctx.begin_path();
    ctx.move_to(x, dims.top_margin);
    ctx.line_to(x, dims.top_margin + dims.graph_height);
    ctx.stroke();

    // Draw draggable handle at top
    ctx.set_fill_style_str(TIME_SCRUBBER_LINE_COLOR);
    ctx.begin_path();
    ctx.move_to(x - TIME_SCRUBBER_HANDLE_SIZE, dims.top_margin - 15.0);
    ctx.line_to(x + TIME_SCRUBBER_HANDLE_SIZE, dims.top_margin - 15.0);
    ctx.line_to(x, dims.top_margin - 5.0);
    ctx.close_path();
    ctx.fill();

    // Draw time label
    ctx.set_fill_style_str(TIME_SCRUBBER_LINE_COLOR);
    ctx.set_font(TIME_SCRUBBER_LABEL_FONT);
    let _ = ctx.fill_text(
        &time.format("%H:%M").to_string(),
        x - 20.0,
        dims.top_margin - 20.0,
    );
}