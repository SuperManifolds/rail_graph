use leptos::*;
use chrono::{NaiveDate, NaiveDateTime};
use web_sys::{MouseEvent, WheelEvent};
use crate::models::{Station, TrainJourney};

#[component]
pub fn GraphCanvas(
    stations: Vec<Station>,
    train_journeys: ReadSignal<Vec<TrainJourney>>,
    visualization_time: ReadSignal<NaiveDateTime>,
    set_visualization_time: WriteSignal<NaiveDateTime>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (zoom_level, set_zoom_level) = create_signal(1.0);
    let (pan_offset_x, set_pan_offset_x) = create_signal(0.0);
    let (pan_offset_y, set_pan_offset_y) = create_signal(0.0);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));

    // Compute conflicts only when train journeys change, not on every render
    let station_names: Vec<String> = stations.iter().map(|s| s.name.clone()).collect();
    let conflicts = create_memo(move |_| {
        let journeys = train_journeys.get();
        crate::components::time_graph::detect_line_conflicts(&journeys, &station_names)
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
            crate::components::time_graph::render_graph(canvas, &stations, &journeys, current, viewport, &current_conflicts);
        }
    });

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
                // Check if click is near the time line (within 10px) for time scrubbing
                let left_margin = 120.0;
                let canvas_width = canvas.width() as f64;
                let graph_width = canvas_width - left_margin - 20.0;

                if x >= left_margin && x <= left_margin + graph_width {
                    set_is_dragging.set(true);
                    update_time_from_x(x, left_margin, graph_width, zoom_level.get(), pan_offset_x.get(), set_visualization_time);
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
                let left_margin = 120.0;
                let canvas_width = canvas.width() as f64;
                let graph_width = canvas_width - left_margin - 20.0;

                if x >= left_margin && x <= left_margin + graph_width {
                    update_time_from_x(x, left_margin, graph_width, zoom_level.get(), pan_offset_x.get(), set_visualization_time);
                }
            }
        }
    };

    let handle_mouse_up = move |_ev: MouseEvent| {
        set_is_dragging.set(false);
        set_is_panning.set(false);
    };

    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = ev.client_x() as f64 - rect.left();
            let mouse_y = ev.client_y() as f64 - rect.top();

            // Only zoom if mouse is within the graph area
            let left_margin = 120.0;
            let top_margin = 60.0;
            let canvas_width = canvas.width() as f64;
            let canvas_height = canvas.height() as f64;
            let graph_width = canvas_width - left_margin - 20.0;
            let graph_height = canvas_height - top_margin - 20.0;

            if mouse_x >= left_margin && mouse_x <= left_margin + graph_width &&
               mouse_y >= top_margin && mouse_y <= top_margin + graph_height {

                let delta = ev.delta_y();
                let zoom_factor = if delta < 0.0 { 1.1 } else { 0.9 };

                let old_zoom = zoom_level.get();
                let new_zoom = (old_zoom * zoom_factor).clamp(0.1, 10.0);

                // Calculate zoom point relative to graph area
                let graph_mouse_x = mouse_x - left_margin;
                let graph_mouse_y = mouse_y - top_margin;

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
        <div class="canvas-container">
            <canvas
                node_ref=canvas_ref
                on:mousedown=handle_mouse_down
                on:mousemove=handle_mouse_move
                on:mouseup=handle_mouse_up
                on:mouseleave=handle_mouse_up
                on:wheel=handle_wheel
                on:contextmenu=|ev| ev.prevent_default()
                style="cursor: crosshair;"
            ></canvas>
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