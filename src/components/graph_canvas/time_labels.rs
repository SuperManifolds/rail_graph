use web_sys::CanvasRenderingContext2d;
use super::types::GraphDimensions;

// Hour grid constants
const HOUR_GRID_COLOR: &str = "#2a2a2a";
const HOUR_GRID_PADDING: i32 = 5;

// Hour label constants
const HOUR_LABEL_COLOR: &str = "#888";
const HOUR_LABEL_FONT: &str = "12px monospace";
const HOUR_LABEL_X_OFFSET: f64 = -15.0;
const HOUR_LABEL_Y_OFFSET_TOP: f64 = -10.0;

// Day indicator constants
const DAY_INDICATOR_COLOR: &str = "#666";
const DAY_INDICATOR_FONT: &str = "10px monospace";
const DAY_INDICATOR_X_OFFSET: f64 = -10.0;
const DAY_INDICATOR_Y_OFFSET: f64 = 5.0;

pub fn draw_hour_grid(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, zoom_level: f64) {
    ctx.set_stroke_style_str(HOUR_GRID_COLOR);
    ctx.set_line_width(1.0 / zoom_level);

    // Calculate visible time range based on current view
    let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
    // Add padding to ensure we draw beyond visible area for smooth panning
    let start_hour = -HOUR_GRID_PADDING;
    let end_hour = hours_visible + HOUR_GRID_PADDING;

    for i in start_hour..=end_hour {
        let x = dims.left_margin + (i as f64 * dims.hour_width);
        draw_vertical_line(ctx, x, dims.top_margin, dims.graph_height);
    }
}

fn draw_vertical_line(ctx: &CanvasRenderingContext2d, x: f64, top: f64, height: f64) {
    ctx.begin_path();
    ctx.move_to(x, top);
    ctx.line_to(x, top + height);
    ctx.stroke();
}

pub fn draw_hour_labels(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    zoom_level: f64,
    pan_offset_x: f64,
) {
    // Calculate which hours are potentially visible
    let start_hour = ((-pan_offset_x) / (dims.hour_width * zoom_level)).floor() as i32 - 1;
    let end_hour =
        ((-pan_offset_x + dims.graph_width) / (dims.hour_width * zoom_level)).ceil() as i32 + 1;

    for i in start_hour..=end_hour {
        let base_x = i as f64 * dims.hour_width;
        let adjusted_x = dims.left_margin + (base_x * zoom_level) + pan_offset_x;

        // Only draw label if it's within the visible graph area
        if adjusted_x >= dims.left_margin
            && adjusted_x <= dims.left_margin + dims.graph_width
            && i >= 0
        {
            let day = i / 24;
            let hour_in_day = i % 24;
            draw_hour_label_with_day(ctx, hour_in_day as usize, day, adjusted_x, dims.top_margin);
        }
    }
}

fn draw_hour_label_with_day(
    ctx: &CanvasRenderingContext2d,
    hour: usize,
    day: i32,
    x: f64,
    top: f64,
) {
    ctx.set_fill_style_str(HOUR_LABEL_COLOR);
    ctx.set_font(HOUR_LABEL_FONT);

    if day == 0 {
        // First day, just show time
        let _ = ctx.fill_text(&format!("{:02}:00", hour), x + HOUR_LABEL_X_OFFSET, top + HOUR_LABEL_Y_OFFSET_TOP);
    } else {
        // Past midnight, show day indicator
        let _ = ctx.fill_text(&format!("{:02}:00", hour), x + HOUR_LABEL_X_OFFSET, top + HOUR_LABEL_Y_OFFSET_TOP);
        ctx.set_font(DAY_INDICATOR_FONT);
        ctx.set_fill_style_str(DAY_INDICATOR_COLOR);
        let _ = ctx.fill_text(&format!("+{}", day), x + DAY_INDICATOR_X_OFFSET, top + DAY_INDICATOR_Y_OFFSET);
    }
}