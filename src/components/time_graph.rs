use leptos::*;
use chrono::{NaiveDate, NaiveDateTime, Timelike};
use web_sys::CanvasRenderingContext2d;
use wasm_bindgen::JsCast;
use crate::models::{Station, TrainJourney};
use crate::utils::{parse_csv_data, generate_train_journeys};
use crate::components::{line_controls::LineControls, graph_canvas::GraphCanvas};
use crate::storage::{save_lines_to_storage, load_lines_from_storage};

#[derive(Clone)]
struct GraphDimensions {
    left_margin: f64,
    top_margin: f64,
    graph_width: f64,
    graph_height: f64,
    hour_width: f64,
}

#[derive(Clone)]
pub struct ViewportState {
    pub zoom_level: f64,
    pub pan_offset_x: f64,
    pub pan_offset_y: f64,
}

impl GraphDimensions {
    fn new(canvas_width: f64, canvas_height: f64) -> Self {
        let left_margin = 120.0;
        let top_margin = 60.0;
        let graph_width = canvas_width - left_margin - 20.0;
        let graph_height = canvas_height - top_margin - 20.0;
        let total_hours = 48.0; // Show 48 hours to support past-midnight

        Self {
            left_margin,
            top_margin,
            graph_width,
            graph_height,
            hour_width: graph_width / total_hours,
        }
    }
}

#[component]
pub fn TimeGraph() -> impl IntoView {
    let (lines_data, stations) = parse_csv_data();

    // Create the main lines signal at the top level
    let (lines, set_lines) = create_signal(lines_data);

    // Auto-load saved configuration on component mount
    create_effect(move |_| {
        if let Ok(saved_lines) = load_lines_from_storage() {
            set_lines.set(saved_lines);
        }
    });

    // Auto-save configuration whenever lines change
    create_effect(move |_| {
        let current_lines = lines.get();
        // Skip saving on initial load to avoid overwriting with default data
        if !current_lines.is_empty() {
            if let Err(e) = save_lines_to_storage(&current_lines) {
                web_sys::console::error_1(&format!("Auto-save failed: {}", e).into());
            }
        }
    });

    let (visualization_time, set_visualization_time) = create_signal(chrono::Local::now().naive_local());
    let (train_journeys, set_train_journeys) = create_signal(Vec::<TrainJourney>::new());

    let stations_clone = stations.clone();

    // Update train journeys only when lines configuration changes
    create_effect(move |_| {
        let current_lines = lines.get();
        let stations_for_journeys = stations_clone.clone();

        // Generate journeys for the full day starting from midnight
        let new_journeys = generate_train_journeys(
            &current_lines,
            &stations_for_journeys,
        );
        set_train_journeys.set(new_journeys);
    });



    view! {
        <div class="time-graph-container">
            <div class="main-content">
                <GraphCanvas
                    stations=stations.clone()
                    train_journeys=train_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
                />
            </div>
            <div class="sidebar">
                <div class="sidebar-header">
                    <h2>"Railway Time Graph"</h2>
                </div>
                <LineControls lines=lines set_lines=set_lines />
            </div>
        </div>
    }
}

pub fn render_graph(
    canvas: leptos::HtmlElement<leptos::html::Canvas>,
    stations: &[Station],
    train_journeys: &[TrainJourney],
    current_time: chrono::NaiveDateTime,
    viewport: ViewportState,
) {
    let canvas_element: &web_sys::HtmlCanvasElement = &canvas;
    let canvas_width = canvas_element.width() as f64;
    let canvas_height = canvas_element.height() as f64;

    let Ok(Some(context)) = canvas_element.get_context("2d") else {
        leptos::logging::warn!("Failed to get 2D context");
        return;
    };

    let Ok(ctx) = context.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        leptos::logging::warn!("Failed to cast to 2D rendering context");
        return;
    };

    // Create dimensions that scale with canvas size
    let dimensions = GraphDimensions::new(canvas_width, canvas_height);

    clear_canvas(&ctx, canvas_width, canvas_height);
    draw_background(&ctx, canvas_width, canvas_height);

    // Apply zoom and pan transformation for all graph content (including grids)
    ctx.save();

    // Clip to graph area only
    ctx.begin_path();
    ctx.rect(dimensions.left_margin, dimensions.top_margin, dimensions.graph_width, dimensions.graph_height);
    ctx.clip();

    // Apply transformation within the clipped area - use canvas scaling but compensate visual elements
    let _ = ctx.translate(dimensions.left_margin, dimensions.top_margin);
    let _ = ctx.translate(viewport.pan_offset_x, viewport.pan_offset_y);
    let _ = ctx.scale(viewport.zoom_level, viewport.zoom_level);

    // Create adjusted dimensions for the zoomed coordinate system
    let mut zoomed_dimensions = dimensions.clone();
    zoomed_dimensions.left_margin = 0.0; // We've already translated to the graph origin
    zoomed_dimensions.top_margin = 0.0;

    // Draw grid and content in zoomed coordinate system
    draw_hour_grid(&ctx, &zoomed_dimensions, viewport.zoom_level);
    let unique_stations = get_visible_stations(stations, stations.len());
    draw_station_grid(&ctx, &zoomed_dimensions, &unique_stations);
    draw_train_journeys(&ctx, &zoomed_dimensions, &unique_stations, train_journeys, current_time, viewport.zoom_level);

    // Restore canvas context
    ctx.restore();

    // Draw labels at normal size but with adjusted positions for zoom/pan
    draw_hour_labels(&ctx, &dimensions, viewport.zoom_level, viewport.pan_offset_x);
    draw_station_labels(&ctx, &dimensions, &unique_stations, viewport.zoom_level, viewport.pan_offset_y);

    // Draw time indicator on top (adjusted for zoom/pan)
    draw_time_indicator(&ctx, &dimensions, current_time, viewport.zoom_level, viewport.pan_offset_x);
}

fn clear_canvas(ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
    ctx.clear_rect(0.0, 0.0, width, height);
}

fn draw_background(ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#0a0a0a"));
    ctx.fill_rect(0.0, 0.0, width, height);
}


fn draw_hour_grid(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, zoom_level: f64) {
    ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str("#2a2a2a"));
    ctx.set_line_width(1.0 / zoom_level);

    // Calculate visible time range based on current view
    let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
    // Add padding to ensure we draw beyond visible area for smooth panning
    let padding_hours = 5;
    let start_hour = -padding_hours;
    let end_hour = hours_visible + padding_hours;

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

fn draw_hour_label_with_day(ctx: &CanvasRenderingContext2d, hour: usize, day: i32, x: f64, top: f64) {
    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#888"));
    ctx.set_font("12px monospace");

    if day == 0 {
        // First day, just show time
        let _ = ctx.fill_text(&format!("{:02}:00", hour), x - 15.0, top - 10.0);
    } else {
        // Past midnight, show day indicator
        let _ = ctx.fill_text(&format!("{:02}:00", hour), x - 15.0, top - 10.0);
        ctx.set_font("10px monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#666"));
        let _ = ctx.fill_text(&format!("+{}", day), x - 10.0, top + 5.0);
    }
}

fn draw_hour_labels(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, zoom_level: f64, pan_offset_x: f64) {
    // Calculate which hours are potentially visible
    let start_hour = ((-pan_offset_x) / (dims.hour_width * zoom_level)).floor() as i32 - 1;
    let end_hour = ((-pan_offset_x + dims.graph_width) / (dims.hour_width * zoom_level)).ceil() as i32 + 1;

    for i in start_hour..=end_hour {
        let base_x = i as f64 * dims.hour_width;
        let adjusted_x = dims.left_margin + (base_x * zoom_level) + pan_offset_x;

        // Only draw label if it's within the visible graph area
        if adjusted_x >= dims.left_margin && adjusted_x <= dims.left_margin + dims.graph_width
            && i >= 0 {
                let day = i / 24;
                let hour_in_day = i % 24;
                draw_hour_label_with_day(ctx, hour_in_day as usize, day, adjusted_x, dims.top_margin);
            }
    }
}

fn draw_station_labels(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, stations: &[String], zoom_level: f64, pan_offset_y: f64) {
    let station_height = dims.graph_height / stations.len() as f64;

    for (i, station) in stations.iter().enumerate() {
        let base_y = (i as f64 * station_height) + (station_height / 2.0);
        let adjusted_y = dims.top_margin + (base_y * zoom_level) + pan_offset_y;

        // Only draw label if it's within the visible graph area
        if adjusted_y >= dims.top_margin && adjusted_y <= dims.top_margin + dims.graph_height {
            draw_station_label(ctx, station, adjusted_y);
        }
    }
}

fn get_visible_stations(stations: &[Station], max_count: usize) -> Vec<String> {
    stations
        .iter()
        .map(|s| s.name.clone())
        .take(max_count)
        .collect()
}


fn draw_station_grid(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
) {
    let station_height = dims.graph_height / stations.len() as f64;

    for (i, _station) in stations.iter().enumerate() {
        let y = calculate_station_y(dims, i, station_height);
        draw_horizontal_line(ctx, dims, y);
    }
}

fn calculate_station_y(dims: &GraphDimensions, index: usize, station_height: f64) -> f64 {
    dims.top_margin + (index as f64 * station_height) + (station_height / 2.0)
}

fn draw_station_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64) {
    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#aaa"));
    ctx.set_font("11px monospace");
    let _ = ctx.fill_text(station, 5.0, y + 3.0);
}

fn draw_horizontal_line(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, y: f64) {
    ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str("#1a1a1a"));
    ctx.begin_path();

    // Calculate the same extended range as the hour grid
    let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
    let padding_hours = 5;
    let start_hour = -padding_hours;
    let end_hour = hours_visible + padding_hours;

    let start_x = dims.left_margin + (start_hour as f64 * dims.hour_width);
    let end_x = dims.left_margin + (end_hour as f64 * dims.hour_width);

    ctx.move_to(start_x, y);
    ctx.line_to(end_x, y);
    ctx.stroke();
}

fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    train_journeys: &[TrainJourney],
    current_time: NaiveDateTime,
    zoom_level: f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    for journey in train_journeys {
        ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str(&journey.color));
        ctx.set_line_width(2.0 / zoom_level);
        ctx.begin_path();

        let mut first_point = true;
        let mut prev_x = 0.0;

        for (station_name, arrival_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s == station_name) {
                let time_fraction = time_to_fraction(*arrival_time);
                let mut x = dims.left_margin + (time_fraction * dims.hour_width);

                // If this x position is much less than the previous x (indicating midnight wrap),
                // add the width of one full day to continue the line
                if !first_point && x < prev_x - dims.graph_width * 0.5 {
                    x += dims.graph_width;
                }
                let y = dims.top_margin + (station_idx as f64 * station_height) + (station_height / 2.0);

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
                if prev_x > 0.0 && x < prev_x - dims.graph_width * 0.5 {
                    x += dims.graph_width;
                }

                let y = dims.top_margin + (station_idx as f64 * station_height) + (station_height / 2.0);

                ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(&journey.color));
                ctx.begin_path();
                let _ = ctx.arc(x, y, 3.0 / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                ctx.fill();

                prev_x = x;
            }
        }
    }

    // Draw current train positions
    draw_current_train_positions(ctx, dims, stations, train_journeys, station_height, current_time, zoom_level);
}

fn draw_current_train_positions(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    train_journeys: &[TrainJourney],
    station_height: f64,
    visualization_time: NaiveDateTime,
    zoom_level: f64,
) {

    for journey in train_journeys {
        // Find which segment the train is currently on
        let mut prev_station: Option<(&String, NaiveDateTime, usize)> = None;
        let mut next_station: Option<(&String, NaiveDateTime, usize)> = None;

        for (station_name, arrival_time) in &journey.station_times {
            if let Some(station_idx) = stations.iter().position(|s| s == station_name) {
                if *arrival_time <= visualization_time {
                    prev_station = Some((station_name, *arrival_time, station_idx));
                } else if next_station.is_none() {
                    next_station = Some((station_name, *arrival_time, station_idx));
                    break;
                }
            }
        }

        // If train is between two stations, interpolate its position
        if let (Some((_, prev_time, prev_idx)), Some((_, next_time, next_idx))) = (prev_station, next_station) {
            let segment_duration = next_time.signed_duration_since(prev_time).num_seconds() as f64;
            let elapsed = visualization_time.signed_duration_since(prev_time).num_seconds() as f64;
            let progress = (elapsed / segment_duration).clamp(0.0, 1.0);

            let prev_x = dims.left_margin + (time_to_fraction(prev_time) * dims.hour_width);
            let prev_y = dims.top_margin + (prev_idx as f64 * station_height) + (station_height / 2.0);

            let next_x = dims.left_margin + (time_to_fraction(next_time) * dims.hour_width);
            let next_y = dims.top_margin + (next_idx as f64 * station_height) + (station_height / 2.0);

            let current_x = prev_x + (next_x - prev_x) * progress;
            let current_y = prev_y + (next_y - prev_y) * progress;

            // Draw train as a larger dot with an outline
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(&journey.color));
            ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str("#fff"));
            ctx.set_line_width(2.0 / zoom_level);
            ctx.begin_path();
            let _ = ctx.arc(current_x, current_y, 6.0 / zoom_level, 0.0, std::f64::consts::PI * 2.0);
            ctx.fill();
            ctx.stroke();

            // Draw train ID label with zoom-compensated font size
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#fff"));
            ctx.set_font(&format!("bold {}px monospace", 10.0 / zoom_level));
            let _ = ctx.fill_text(&journey.line_id, current_x - 12.0 / zoom_level, current_y - 10.0 / zoom_level);
        }
    }
}

fn time_to_fraction(time: chrono::NaiveDateTime) -> f64 {
    // Calculate hours from the base date (2024-01-01 00:00:00)
    let base_date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("Valid date");
    let base_datetime = base_date.and_hms_opt(0, 0, 0).expect("Valid datetime");

    let duration_since_base = time.signed_duration_since(base_datetime);
    let total_seconds = duration_since_base.num_seconds() as f64;
    total_seconds / 3600.0 // Convert to hours
}

fn draw_time_indicator(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    time: NaiveDateTime,
    zoom_level: f64,
    pan_offset_x: f64,
) {
    let time_fraction = time_to_fraction(time);
    let base_x = time_fraction * dims.hour_width;
    let x = dims.left_margin + (base_x * zoom_level) + pan_offset_x;

    // Only draw if the time indicator is within the visible graph area
    if x < dims.left_margin || x > dims.left_margin + dims.graph_width {
        return;
    }

    // Draw semi-transparent background for the line
    ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str("rgba(255, 51, 51, 0.3)"));
    ctx.set_line_width(8.0);
    ctx.begin_path();
    ctx.move_to(x, dims.top_margin);
    ctx.line_to(x, dims.top_margin + dims.graph_height);
    ctx.stroke();

    // Draw main line
    ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str("#FF3333"));
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(x, dims.top_margin);
    ctx.line_to(x, dims.top_margin + dims.graph_height);
    ctx.stroke();

    // Draw draggable handle at top
    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#FF3333"));
    ctx.begin_path();
    ctx.move_to(x - 8.0, dims.top_margin - 15.0);
    ctx.line_to(x + 8.0, dims.top_margin - 15.0);
    ctx.line_to(x, dims.top_margin - 5.0);
    ctx.close_path();
    ctx.fill();

    // Draw time label
    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#FF3333"));
    ctx.set_font("bold 12px monospace");
    let _ = ctx.fill_text(
        &time.format("%H:%M").to_string(),
        x - 20.0,
        dims.top_margin - 20.0
    );
}


