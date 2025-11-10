use super::types::GraphDimensions;
use web_sys::CanvasRenderingContext2d;
use crate::theme::Theme;

const HOUR_GRID_PADDING: i32 = 5;

// Zoom thresholds for showing subdivisions (effective pixels per hour)
const TEN_MIN_THRESHOLD: f64 = 150.0;
const MINUTE_THRESHOLD: f64 = 1250.0;

// Hour label constants
const HOUR_LABEL_FONT: &str = "12px monospace";
const HOUR_LABEL_X_OFFSET: f64 = -15.0;
const HOUR_LABEL_Y_OFFSET_TOP: f64 = -10.0;

// Sub-hour label constants
const TEN_MIN_LABEL_FONT: &str = "10px monospace";
const TEN_MIN_LABEL_X_OFFSET: f64 = -8.0;
const MINUTE_LABEL_FONT: &str = "9px monospace";
const MINUTE_LABEL_X_OFFSET: f64 = -6.0;
const MINUTE_LABEL_BOLD_FONT: &str = "bold 10px monospace";

// Day indicator constants
const DAY_INDICATOR_FONT: &str = "10px monospace";
const DAY_INDICATOR_X_OFFSET: f64 = -10.0;
const DAY_INDICATOR_Y_OFFSET: f64 = 5.0;

struct Palette {
    hour_grid: &'static str,
    ten_min_grid: &'static str,
    minute_grid: &'static str,
    hour_label: &'static str,
    ten_min_label: &'static str,
    minute_label: &'static str,
    minute_label_bold: &'static str,
    day_indicator: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    hour_grid: "#2a2a2a",
    ten_min_grid: "#1a1a1a",
    minute_grid: "#151515",
    hour_label: "#888",
    ten_min_label: "#666",
    minute_label: "#555",
    minute_label_bold: "#777",
    day_indicator: "#666",
};

const LIGHT_PALETTE: Palette = Palette {
    hour_grid: "#d0d0d0",
    ten_min_grid: "#e5e5e5",
    minute_grid: "#ececec",
    hour_label: "#666",
    ten_min_label: "#999",
    minute_label: "#aaa",
    minute_label_bold: "#888",
    day_indicator: "#999",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

#[allow(clippy::cast_possible_truncation)]
pub fn draw_hour_grid(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    theme: Theme,
) {
    let palette = get_palette(theme);
    let line_width = 1.0 / zoom_level;

    // Calculate visible range in the transformed coordinate system
    let x_min = -pan_offset_x / zoom_level;
    let x_max = (dims.graph_width - pan_offset_x) / zoom_level;

    // Determine which subdivisions to show based on effective hour width
    let effective_hour_width = dims.hour_width * zoom_level_x;
    let show_ten_min = effective_hour_width > TEN_MIN_THRESHOLD;
    let show_minutes = effective_hour_width > MINUTE_THRESHOLD;

    // Draw hour lines
    ctx.set_stroke_style_str(palette.hour_grid);
    ctx.set_line_width(line_width);
    let start_hour = (x_min / dims.hour_width).floor() as i32 - HOUR_GRID_PADDING;
    let end_hour = (x_max / dims.hour_width).ceil() as i32 + HOUR_GRID_PADDING;

    for i in start_hour..=end_hour {
        let x = dims.left_margin + (f64::from(i) * dims.hour_width);
        draw_vertical_line(ctx, x, dims.top_margin, dims.graph_height);
    }

    // Draw 10-minute or minute subdivisions if zoomed in enough
    if show_minutes {
        // Draw minute lines (60 per hour)
        let minute_width = dims.hour_width / 60.0;
        let start_min = (x_min / minute_width).floor() as i32 - HOUR_GRID_PADDING * 60;
        let end_min = (x_max / minute_width).ceil() as i32 + HOUR_GRID_PADDING * 60;

        for i in start_min..=end_min {
            if i % 60 != 0 {
                // Skip hour marks
                let x = dims.left_margin + (f64::from(i) * minute_width);

                // Make 10-minute marks bolder
                if i % 10 == 0 {
                    ctx.set_stroke_style_str(palette.ten_min_grid);
                    ctx.set_line_width(line_width * 0.7);
                } else {
                    ctx.set_stroke_style_str(palette.minute_grid);
                    ctx.set_line_width(line_width * 0.5);
                }

                draw_vertical_line(ctx, x, dims.top_margin, dims.graph_height);
            }
        }
    } else if show_ten_min {
        // Draw 10-minute lines (6 per hour)
        ctx.set_stroke_style_str(palette.ten_min_grid);
        ctx.set_line_width(line_width * 0.7);
        let ten_min_width = dims.hour_width / 6.0;
        let start_ten_min = (x_min / ten_min_width).floor() as i32 - HOUR_GRID_PADDING * 6;
        let end_ten_min = (x_max / ten_min_width).ceil() as i32 + HOUR_GRID_PADDING * 6;

        for i in start_ten_min..=end_ten_min {
            if i % 6 != 0 {
                // Skip hour marks
                let x = dims.left_margin + (f64::from(i) * ten_min_width);
                draw_vertical_line(ctx, x, dims.top_margin, dims.graph_height);
            }
        }
    }
}

fn draw_vertical_line(ctx: &CanvasRenderingContext2d, x: f64, top: f64, height: f64) {
    ctx.begin_path();
    ctx.move_to(x, top);
    ctx.line_to(x, top + height);
    ctx.stroke();
}

#[allow(clippy::cast_possible_truncation)]
pub fn draw_hour_labels(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    theme: Theme,
) {
    let palette = get_palette(theme);

    // Account for both uniform zoom and horizontal zoom
    let effective_hour_width = dims.hour_width * zoom_level * zoom_level_x;

    // Determine which subdivisions to show
    let show_ten_min = effective_hour_width > TEN_MIN_THRESHOLD;
    let show_minutes = effective_hour_width > MINUTE_THRESHOLD;

    // Draw hour labels
    let start_hour = ((-pan_offset_x) / effective_hour_width).floor() as i32 - 1;
    let end_hour = ((-pan_offset_x + dims.graph_width) / effective_hour_width).ceil() as i32 + 1;

    for i in start_hour..=end_hour {
        let base_x = f64::from(i) * dims.hour_width;
        let adjusted_x = dims.left_margin + (base_x * zoom_level * zoom_level_x) + pan_offset_x;

        if adjusted_x >= dims.left_margin
            && adjusted_x <= dims.left_margin + dims.graph_width
            && i >= 0
        {
            let day = i / 24;
            let hour_in_day = i % 24;
            draw_hour_label_with_day(ctx, hour_in_day, day, adjusted_x, dims.top_margin, palette);
        }
    }

    // Draw subdivision labels if zoomed in enough
    if show_minutes {
        // Draw minute labels
        let minute_width = dims.hour_width / 60.0;
        let effective_minute_width = minute_width * zoom_level * zoom_level_x;
        let start_min = ((-pan_offset_x) / effective_minute_width).floor() as i32 - 1;
        let end_min =
            ((-pan_offset_x + dims.graph_width) / effective_minute_width).ceil() as i32 + 1;

        for i in start_min..=end_min {
            if i % 60 != 0 && i >= 0 {
                // Skip hour marks and negative
                let base_x = f64::from(i) * minute_width;
                let adjusted_x =
                    dims.left_margin + (base_x * zoom_level * zoom_level_x) + pan_offset_x;

                if adjusted_x >= dims.left_margin
                    && adjusted_x <= dims.left_margin + dims.graph_width
                {
                    let minute = i % 60;
                    draw_minute_label(ctx, minute, adjusted_x, dims.top_margin, palette);
                }
            }
        }
    } else if show_ten_min {
        // Draw 10-minute labels
        let ten_min_width = dims.hour_width / 6.0;
        let effective_ten_min_width = ten_min_width * zoom_level * zoom_level_x;
        let start_ten_min = ((-pan_offset_x) / effective_ten_min_width).floor() as i32 - 1;
        let end_ten_min =
            ((-pan_offset_x + dims.graph_width) / effective_ten_min_width).ceil() as i32 + 1;

        for i in start_ten_min..=end_ten_min {
            if i % 6 != 0 && i >= 0 {
                // Skip hour marks and negative
                let base_x = f64::from(i) * ten_min_width;
                let adjusted_x =
                    dims.left_margin + (base_x * zoom_level * zoom_level_x) + pan_offset_x;

                if adjusted_x >= dims.left_margin
                    && adjusted_x <= dims.left_margin + dims.graph_width
                {
                    let ten_minutes = (i % 6) * 10;
                    draw_ten_min_label(ctx, ten_minutes, adjusted_x, dims.top_margin, palette);
                }
            }
        }
    }
}

fn draw_hour_label_with_day(
    ctx: &CanvasRenderingContext2d,
    hour: i32,
    day: i32,
    x: f64,
    top: f64,
    palette: &Palette,
) {
    ctx.set_fill_style_str(palette.hour_label);
    ctx.set_font(HOUR_LABEL_FONT);

    if day == 0 {
        // First day, just show time
        let _ = ctx.fill_text(
            &format!("{hour:02}:00"),
            x + HOUR_LABEL_X_OFFSET,
            top + HOUR_LABEL_Y_OFFSET_TOP,
        );
    } else {
        // Past midnight, show day indicator
        let _ = ctx.fill_text(
            &format!("{hour:02}:00"),
            x + HOUR_LABEL_X_OFFSET,
            top + HOUR_LABEL_Y_OFFSET_TOP,
        );
        ctx.set_font(DAY_INDICATOR_FONT);
        ctx.set_fill_style_str(palette.day_indicator);
        let _ = ctx.fill_text(
            &format!("+{day}"),
            x + DAY_INDICATOR_X_OFFSET,
            top + DAY_INDICATOR_Y_OFFSET,
        );
    }
}

fn draw_ten_min_label(ctx: &CanvasRenderingContext2d, ten_minutes: i32, x: f64, top: f64, palette: &Palette) {
    ctx.set_fill_style_str(palette.ten_min_label);
    ctx.set_font(TEN_MIN_LABEL_FONT);
    let _ = ctx.fill_text(
        &format!(":{ten_minutes:02}"),
        x + TEN_MIN_LABEL_X_OFFSET,
        top + HOUR_LABEL_Y_OFFSET_TOP,
    );
}

fn draw_minute_label(ctx: &CanvasRenderingContext2d, minute: i32, x: f64, top: f64, palette: &Palette) {
    // Make 10-minute marks bolder
    if minute % 10 == 0 {
        ctx.set_fill_style_str(palette.minute_label_bold);
        ctx.set_font(MINUTE_LABEL_BOLD_FONT);
        let _ = ctx.fill_text(
            &format!(":{minute:02}"),
            x + TEN_MIN_LABEL_X_OFFSET,
            top + HOUR_LABEL_Y_OFFSET_TOP,
        );
    } else {
        ctx.set_fill_style_str(palette.minute_label);
        ctx.set_font(MINUTE_LABEL_FONT);
        let _ = ctx.fill_text(
            &format!(":{minute:02}"),
            x + MINUTE_LABEL_X_OFFSET,
            top + HOUR_LABEL_Y_OFFSET_TOP,
        );
    }
}
