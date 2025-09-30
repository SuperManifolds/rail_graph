use crate::components::{
    graph_canvas::{
        GraphCanvas, GraphDimensions, ViewportState,
        station_labels, time_labels, graph_content,
        Conflict,
    },
    line_controls::LineControls
};
use crate::models::{SegmentState, Station, TrainJourney};
use crate::storage::{
    load_lines_from_storage, load_segment_state_from_storage, save_lines_to_storage,
    save_segment_state_to_storage,
};
use crate::data::parse_csv_data;
use leptos::*;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;

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

    let (visualization_time, set_visualization_time) =
        create_signal(chrono::Local::now().naive_local());
    let (train_journeys, set_train_journeys) = create_signal(Vec::<TrainJourney>::new());

    // Segment state for double tracking
    let (segment_state, set_segment_state) = create_signal(SegmentState {
        double_tracked_segments: HashSet::new(),
    });

    // Auto-load saved segment state on component mount
    create_effect(move |_| {
        match load_segment_state_from_storage() {
            Ok(saved_state) => {
                set_segment_state.set(saved_state);
            }
            Err(_) => {
                // If no saved state found, use default empty state
                set_segment_state.set(SegmentState {
                    double_tracked_segments: HashSet::new(),
                });
            }
        }
    });

    // Auto-save segment state whenever it changes
    create_effect(move |_| {
        let current_state = segment_state.get();
        if let Err(e) = save_segment_state_to_storage(&current_state) {
            web_sys::console::error_1(&format!("Auto-save segment state failed: {}", e).into());
        }
    });

    let stations_clone = stations.clone();

    // Update train journeys only when lines configuration changes
    create_effect(move |_| {
        let current_lines = lines.get();
        let stations_for_journeys = stations_clone.clone();

        // Generate journeys for the full day starting from midnight
        let new_journeys = TrainJourney::generate_journeys(&current_lines, &stations_for_journeys);
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
                    segment_state=segment_state
                    set_segment_state=set_segment_state
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
    conflicts: &[Conflict],
    segment_state: &SegmentState,
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
    graph_content::draw_background(&ctx, canvas_width, canvas_height);

    // Apply zoom and pan transformation for all graph content (including grids)
    ctx.save();

    // Clip to graph area only
    ctx.begin_path();
    ctx.rect(
        dimensions.left_margin,
        dimensions.top_margin,
        dimensions.graph_width,
        dimensions.graph_height,
    );
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
    time_labels::draw_hour_grid(&ctx, &zoomed_dimensions, viewport.zoom_level);
    let unique_stations = get_visible_stations(stations, stations.len());
    graph_content::draw_station_grid(&ctx, &zoomed_dimensions, &unique_stations);
    graph_content::draw_double_track_indicators(&ctx, &zoomed_dimensions, &unique_stations, segment_state);

    // Draw train journeys
    let station_height = zoomed_dimensions.graph_height / unique_stations.len() as f64;
    graph_content::draw_train_journeys(
        &ctx,
        &zoomed_dimensions,
        &unique_stations,
        train_journeys,
        viewport.zoom_level,
    );

    // Draw conflicts
    graph_content::draw_conflict_highlights(
        &ctx,
        &zoomed_dimensions,
        conflicts,
        station_height,
        viewport.zoom_level,
    );

    // Draw current train positions
    graph_content::draw_current_train_positions(
        &ctx,
        &zoomed_dimensions,
        &unique_stations,
        train_journeys,
        station_height,
        current_time,
        viewport.zoom_level,
    );

    // Restore canvas context
    ctx.restore();

    // Draw labels at normal size but with adjusted positions for zoom/pan
    time_labels::draw_hour_labels(
        &ctx,
        &dimensions,
        viewport.zoom_level,
        viewport.pan_offset_x,
    );
    station_labels::draw_station_labels(
        &ctx,
        &dimensions,
        &unique_stations,
        viewport.zoom_level,
        viewport.pan_offset_y,
    );
    station_labels::draw_segment_toggles(
        &ctx,
        &dimensions,
        &unique_stations,
        segment_state,
        viewport.zoom_level,
        viewport.pan_offset_y,
    );

    // Draw time indicator on top (adjusted for zoom/pan)
    graph_content::draw_time_indicator(
        &ctx,
        &dimensions,
        current_time,
        viewport.zoom_level,
        viewport.pan_offset_x,
    );
}

fn clear_canvas(ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
    ctx.clear_rect(0.0, 0.0, width, height);
}

fn get_visible_stations(stations: &[Station], max_count: usize) -> Vec<String> {
    stations
        .iter()
        .map(|s| s.name.clone())
        .take(max_count)
        .collect()
}
