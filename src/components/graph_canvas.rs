use leptos::*;
use chrono::{NaiveDate, NaiveDateTime};
use web_sys::{MouseEvent, WheelEvent};
use crate::models::{Station, TrainJourney, SegmentState};
use crate::components::conflict_tooltip::{ConflictTooltip, check_conflict_hover};

// Layout constants for the graph canvas
pub const LEFT_MARGIN: f64 = 120.0;
pub const TOP_MARGIN: f64 = 60.0;
pub const RIGHT_PADDING: f64 = 20.0;
pub const BOTTOM_PADDING: f64 = 20.0;
const TOGGLE_X: f64 = 85.0;
const TOGGLE_SIZE: f64 = 12.0;

#[component]
pub fn GraphCanvas(
    stations: Vec<Station>,
    train_journeys: ReadSignal<Vec<TrainJourney>>,
    visualization_time: ReadSignal<NaiveDateTime>,
    set_visualization_time: WriteSignal<NaiveDateTime>,
    segment_state: ReadSignal<SegmentState>,
    set_segment_state: WriteSignal<SegmentState>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (zoom_level, set_zoom_level) = create_signal(1.0);
    let (pan_offset_x, set_pan_offset_x) = create_signal(0.0);
    let (pan_offset_y, set_pan_offset_y) = create_signal(0.0);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));
    let (hovered_conflict, set_hovered_conflict) = create_signal(None::<(crate::components::time_graph::Conflict, f64, f64)>);

    // Clone stations for use in render closure
    let stations_for_render = stations.clone();

    // Compute conflicts only when train journeys change, not on every render
    let station_names: Vec<String> = stations.iter().map(|s| s.name.clone()).collect();
    let conflicts = create_memo(move |_| {
        let journeys = train_journeys.get();
        let seg_state = segment_state.get();
        crate::components::time_graph::detect_line_conflicts(&journeys, &station_names, &seg_state)
    });

    // Render the graph whenever train journeys change
    create_effect(move |_| {
        let journeys = train_journeys.get();
        let current = visualization_time.get();

        if let Some(canvas) = canvas_ref.get() {
            let zoom = zoom_level.get();
            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();

            // Update canvas size to match container
            let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
            let container_width = canvas_elem.client_width() as u32;
            let container_height = canvas_elem.client_height() as u32;

            if container_width > 0 && container_height > 0 {
                canvas_elem.set_width(container_width);
                canvas_elem.set_height(container_height);
            }

            let viewport = crate::components::time_graph::ViewportState {
                zoom_level: zoom,
                pan_offset_x: pan_x,
                pan_offset_y: pan_y,
            };
            let current_conflicts = conflicts.get();
            let current_segment_state = segment_state.get();
            crate::components::time_graph::render_graph(canvas, &stations_for_render, &journeys, current, viewport, &current_conflicts, &current_segment_state);
        }
    });

    // Clone stations for closures
    let stations_for_mouse_down = stations.clone();
    let stations_for_mouse_move = stations.clone();

    // Handle mouse events for dragging the time indicator and panning
    let handle_mouse_down = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = ev.client_x() as f64 - rect.left();
            let y = ev.client_y() as f64 - rect.top();

            // If right click or ctrl+click, start panning
            if ev.button() == 2 || ev.ctrl_key() {
                set_is_panning.set(true);
                set_last_mouse_pos.set((x, y));
            } else {
                // Check for toggle button clicks first
                let canvas_width = canvas.width() as f64;
                let canvas_height = canvas.height() as f64;

                if let Some(clicked_segment) = check_toggle_click(
                    x, y, canvas_height, &stations_for_mouse_down,
                    zoom_level.get(), pan_offset_y.get()
                ) {
                    // Toggle the segment state
                    set_segment_state.update(move |state| {
                        if state.double_tracked_segments.contains(&clicked_segment) {
                            state.double_tracked_segments.remove(&clicked_segment);
                        } else {
                            state.double_tracked_segments.insert(clicked_segment);
                        }
                    });
                } else {
                    // Check if click is near the time line (within 10px) for time scrubbing
                    let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;

                    if x >= LEFT_MARGIN && x <= LEFT_MARGIN + graph_width {
                        set_is_dragging.set(true);
                        update_time_from_x(x, LEFT_MARGIN, graph_width, zoom_level.get(), pan_offset_x.get(), set_visualization_time);
                    }
                }
            }
        }
    };

    let handle_mouse_move = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = ev.client_x() as f64 - rect.left();
            let y = ev.client_y() as f64 - rect.top();

            if is_panning.get() {
                let (last_x, last_y) = last_mouse_pos.get();
                let dx = x - last_x;
                let dy = y - last_y;

                let current_pan_x = pan_offset_x.get();
                let current_pan_y = pan_offset_y.get();

                set_pan_offset_x.set(current_pan_x + dx);
                set_pan_offset_y.set(current_pan_y + dy);
                set_last_mouse_pos.set((x, y));
            } else if is_dragging.get() {
                let canvas_width = canvas.width() as f64;
                let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;

                if x >= LEFT_MARGIN && x <= LEFT_MARGIN + graph_width {
                    update_time_from_x(x, LEFT_MARGIN, graph_width, zoom_level.get(), pan_offset_x.get(), set_visualization_time);
                }
            } else {
                // Check for conflict hover
                let current_conflicts = conflicts.get();
                let hovered = check_conflict_hover(
                    x, y, &current_conflicts, &stations_for_mouse_move,
                    canvas.width() as f64, canvas.height() as f64,
                    zoom_level.get(), pan_offset_x.get(), pan_offset_y.get()
                );
                set_hovered_conflict.set(hovered);
            }
        }
    };

    let handle_mouse_up = move |_ev: MouseEvent| {
        set_is_dragging.set(false);
        set_is_panning.set(false);
    };

    let handle_mouse_leave = move |_ev: MouseEvent| {
        set_is_dragging.set(false);
        set_is_panning.set(false);
        set_hovered_conflict.set(None);
    };

    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = ev.client_x() as f64 - rect.left();
            let mouse_y = ev.client_y() as f64 - rect.top();

            // Only zoom if mouse is within the graph area
            let canvas_width = canvas.width() as f64;
            let canvas_height = canvas.height() as f64;
            let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;
            let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;

            if mouse_x >= LEFT_MARGIN && mouse_x <= LEFT_MARGIN + graph_width &&
               mouse_y >= TOP_MARGIN && mouse_y <= TOP_MARGIN + graph_height {

                let delta = ev.delta_y();
                let zoom_factor = if delta < 0.0 { 1.1 } else { 0.9 };

                let old_zoom = zoom_level.get();
                let new_zoom = (old_zoom * zoom_factor).clamp(0.1, 10.0);

                // Calculate zoom point relative to graph area
                let graph_mouse_x = mouse_x - LEFT_MARGIN;
                let graph_mouse_y = mouse_y - TOP_MARGIN;

                // Zoom towards mouse position within graph
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();

                let new_pan_x = graph_mouse_x - (graph_mouse_x - pan_x) * (new_zoom / old_zoom);
                let new_pan_y = graph_mouse_y - (graph_mouse_y - pan_y) * (new_zoom / old_zoom);

                set_zoom_level.set(new_zoom);
                set_pan_offset_x.set(new_pan_x);
                set_pan_offset_y.set(new_pan_y);
            }
        }
    };

    view! {
        <div class="canvas-container" style="position: relative;">
            <canvas
                node_ref=canvas_ref
                on:mousedown=handle_mouse_down
                on:mousemove=handle_mouse_move
                on:mouseup=handle_mouse_up
                on:mouseleave=handle_mouse_leave
                on:wheel=handle_wheel
                on:contextmenu=|ev| ev.prevent_default()
                style="cursor: crosshair;"
            ></canvas>

            <ConflictTooltip hovered_conflict=hovered_conflict />
        </div>
    }
}

fn update_time_from_x(x: f64, left_margin: f64, graph_width: f64, zoom_level: f64, pan_offset_x: f64, set_time: WriteSignal<NaiveDateTime>) {
    // Transform mouse coordinates to account for zoom and pan
    // Reverse the transformations applied in render_graph:
    // 1. Remove left margin offset to get graph-relative position
    let graph_x = x - left_margin;
    // 2. Account for pan offset (subtract because pan moves the content)
    let panned_x = graph_x - pan_offset_x;
    // 3. Account for zoom (divide because zoom scales the content up)
    let zoomed_x = panned_x / zoom_level;

    // Now calculate fraction based on the base (unzoomed) graph width
    let base_graph_width = graph_width;
    let fraction = zoomed_x / base_graph_width;

    let total_hours = fraction * 48.0; // 48 hours to support past-midnight
    let total_minutes = (total_hours * 60.0) as u32;

    // Calculate days, hours, and minutes
    let days = total_minutes / (24 * 60);
    let remaining_minutes = total_minutes % (24 * 60);
    let hours = remaining_minutes / 60;
    let minutes = remaining_minutes % 60;

    let base_date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("Valid date");
    let target_date = base_date + chrono::Duration::days(days as i64);

    if let Some(new_time) = chrono::NaiveTime::from_hms_opt(hours, minutes, 0) {
        let new_datetime = target_date.and_time(new_time);
        set_time.set(new_datetime);
    }
}

fn check_toggle_click(
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
    if mouse_x >= TOGGLE_X - TOGGLE_SIZE/2.0 && mouse_x <= TOGGLE_X + TOGGLE_SIZE/2.0 {
        // Check each segment toggle
        for i in 1..stations.len() {
            let segment_index = i;

            // Calculate position between the two stations (same logic as draw_segment_toggles)
            let base_y1 = ((i - 1) as f64 * station_height) + (station_height / 2.0);
            let base_y2 = (i as f64 * station_height) + (station_height / 2.0);
            let center_y = (base_y1 + base_y2) / 2.0;
            let adjusted_y = TOP_MARGIN + (center_y * zoom_level) + pan_offset_y;

            // Check if click is within this toggle button
            if mouse_y >= adjusted_y - TOGGLE_SIZE/2.0 && mouse_y <= adjusted_y + TOGGLE_SIZE/2.0 {
                return Some(segment_index);
            }
        }
    }

    None
}

