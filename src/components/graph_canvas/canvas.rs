use leptos::*;
use chrono::NaiveDateTime;
use web_sys::{MouseEvent, WheelEvent, CanvasRenderingContext2d};
use wasm_bindgen::{JsCast, closure::Closure};
use crate::models::{Conflict, StationCrossing, RailwayGraph, TrainJourney};
use crate::components::conflict_tooltip::ConflictTooltip;
use crate::constants::BASE_DATE;
use crate::time::time_to_fraction;
use super::{station_labels, time_labels, conflict_indicators, train_positions, train_journeys, time_scrubber, graph_content};
use super::types::{GraphDimensions, ViewportState};

// Layout constants for the graph canvas
pub const LEFT_MARGIN: f64 = 120.0;
pub const TOP_MARGIN: f64 = 60.0;
pub const RIGHT_PADDING: f64 = 20.0;
pub const BOTTOM_PADDING: f64 = 20.0;

#[component]
pub fn GraphCanvas(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    train_journeys: ReadSignal<Vec<TrainJourney>>,
    visualization_time: ReadSignal<NaiveDateTime>,
    set_visualization_time: WriteSignal<NaiveDateTime>,
    show_station_crossings: ReadSignal<bool>,
    show_conflicts: ReadSignal<bool>,
    conflicts_and_crossings: Memo<(Vec<Conflict>, Vec<StationCrossing>)>,
    #[prop(optional)] pan_to_conflict_signal: Option<ReadSignal<Option<(f64, f64)>>>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (zoom_level, set_zoom_level) = create_signal(1.0);
    let (zoom_level_x, set_zoom_level_x) = create_signal(1.0); // Horizontal (time) zoom
    let (pan_offset_x, set_pan_offset_x) = create_signal(0.0);
    let (pan_offset_y, set_pan_offset_y) = create_signal(0.0);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));
    let (hovered_conflict, set_hovered_conflict) = create_signal(None::<(crate::models::Conflict, f64, f64)>);

    // Handle pan to conflict requests
    if let Some(pan_signal) = pan_to_conflict_signal {
        create_effect(move |_| {
            if let Some((time_fraction, station_pos)) = pan_signal.get() {
                if let Some(canvas_elem) = canvas_ref.get() {
                    let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
                    let canvas_width = canvas.width() as f64;
                    let canvas_height = canvas.height() as f64;

                    let dims = GraphDimensions::new(canvas_width, canvas_height);

                    let current_graph = graph.get();
                    let station_count = current_graph.get_all_stations_ordered().len() as f64;

                    // Set a comfortable zoom level for viewing conflicts
                    let target_zoom = 4.0;
                    set_zoom_level.set(target_zoom);

                    // Center the conflict in the viewport with zoom applied
                    let target_x = (time_fraction * dims.hour_width * target_zoom) - (canvas_width / 2.0);
                    let target_y = (station_pos * (dims.graph_height / station_count.max(1.0)) * target_zoom) - (canvas_height / 2.0);

                    set_pan_offset_x.set(-target_x);
                    set_pan_offset_y.set(-target_y);
                }
            }
        });
    }

    // Render the graph whenever train journeys or graph change
    // Use requestAnimationFrame to throttle renders to 60fps max
    let (render_requested, set_render_requested) = create_signal(false);

    create_effect(move |_| {
        // Track all dependencies
        let _ = train_journeys.get();
        let _ = visualization_time.get();
        let _ = graph.get();
        let _ = zoom_level.get();
        let _ = zoom_level_x.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();
        let _ = conflicts_and_crossings.get();
        let _ = show_station_crossings.get();
        let _ = show_conflicts.get();

        // Only request render if one isn't already pending
        if !render_requested.get_untracked() {
            set_render_requested.set(true);

            let window = web_sys::window().expect("window");
            let callback = Closure::once(move || {
                set_render_requested.set(false);

                // Perform actual render
                let journeys = train_journeys.get_untracked();
                let current = visualization_time.get_untracked();
                let current_graph = graph.get_untracked();
                let stations_for_render = current_graph.get_all_stations_ordered();

                if let Some(canvas) = canvas_ref.get_untracked() {
                    let zoom = zoom_level.get_untracked();
                    let zoom_x = zoom_level_x.get_untracked();
                    let pan_x = pan_offset_x.get_untracked();
                    let pan_y = pan_offset_y.get_untracked();

                    // Update canvas size to match container
                    let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
                    let container_width = canvas_elem.client_width() as u32;
                    let container_height = canvas_elem.client_height() as u32;

                    if container_width > 0 && container_height > 0 {
                        canvas_elem.set_width(container_width);
                        canvas_elem.set_height(container_height);
                    }

                    let viewport = ViewportState {
                        zoom_level: zoom,
                        zoom_level_x: zoom_x,
                        pan_offset_x: pan_x,
                        pan_offset_y: pan_y,
                    };
                    let (current_conflicts, current_station_crossings) = conflicts_and_crossings.get_untracked();
                    let show_crossings = show_station_crossings.get_untracked();
                    let show_conf = show_conflicts.get_untracked();
                    render_graph(canvas, &stations_for_render, &journeys, current, viewport, &current_conflicts, &current_station_crossings, &current_graph, show_crossings, show_conf);
                }
            });

            let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
            callback.forget();
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
                // Check for toggle button clicks first
                let canvas_width = canvas.width() as f64;
                let canvas_height = canvas.height() as f64;
                let current_graph = graph.get();
                let current_stations = current_graph.get_all_stations_ordered();

                if let Some(clicked_segment) = station_labels::check_toggle_click(
                    x, y, canvas_height, &current_stations,
                    zoom_level.get(), pan_offset_y.get()
                ) {
                    toggle_segment_double_track(clicked_segment, &current_stations, set_graph);
                } else {
                    handle_time_scrubbing(x, canvas_width, zoom_level.get(), zoom_level_x.get(), pan_offset_x.get(), set_is_dragging, set_visualization_time);
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

                // Batch pan updates to trigger only one re-render
                batch(move || {
                    set_pan_offset_x.set(current_pan_x + dx);
                    set_pan_offset_y.set(current_pan_y + dy);
                    set_last_mouse_pos.set((x, y));
                });
            } else if is_dragging.get() {
                let canvas_width = canvas.width() as f64;
                let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;

                if x >= LEFT_MARGIN && x <= LEFT_MARGIN + graph_width {
                    update_time_from_x(x, LEFT_MARGIN, graph_width, zoom_level.get(), zoom_level_x.get(), pan_offset_x.get(), set_visualization_time);
                }
            } else {
                // Check for conflict hover
                let (current_conflicts, _) = conflicts_and_crossings.get();
                let current_graph = graph.get();
                let current_stations = current_graph.get_all_stations_ordered();
                let hovered = conflict_indicators::check_conflict_hover(
                    x, y, &current_conflicts, &current_stations,
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
                let shift_pressed = ev.shift_key();

                // Calculate zoom point relative to graph area
                let graph_mouse_x = mouse_x - LEFT_MARGIN;
                let graph_mouse_y = mouse_y - TOP_MARGIN;

                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();

                if shift_pressed {
                    // Horizontal (time) zoom only
                    let old_zoom_x = zoom_level_x.get();
                    let new_zoom_x = (old_zoom_x * zoom_factor).clamp(0.1, 25.0);
                    let new_pan_x = graph_mouse_x - (graph_mouse_x - pan_x) * (new_zoom_x / old_zoom_x);

                    batch(move || {
                        set_zoom_level_x.set(new_zoom_x);
                        set_pan_offset_x.set(new_pan_x);
                    });
                } else {
                    // Normal zoom (both dimensions)
                    let old_zoom = zoom_level.get();
                    let new_zoom = (old_zoom * zoom_factor).clamp(0.1, 25.0);

                    let new_pan_x = graph_mouse_x - (graph_mouse_x - pan_x) * (new_zoom / old_zoom);
                    let new_pan_y = graph_mouse_y - (graph_mouse_y - pan_y) * (new_zoom / old_zoom);

                    batch(move || {
                        set_zoom_level.set(new_zoom);
                        set_pan_offset_x.set(new_pan_x);
                        set_pan_offset_y.set(new_pan_y);
                    });
                }
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

fn update_time_from_x(x: f64, left_margin: f64, graph_width: f64, zoom_level: f64, zoom_level_x: f64, pan_offset_x: f64, set_time: WriteSignal<NaiveDateTime>) {
    // Transform mouse coordinates to account for zoom and pan
    // Reverse the transformations applied in render_graph:
    // 1. Remove left margin offset to get graph-relative position
    let graph_x = x - left_margin;
    // 2. Account for pan offset (subtract because pan moves the content)
    let panned_x = graph_x - pan_offset_x;
    // 3. Account for uniform zoom (divide because zoom scales the content up)
    let uniformly_unzoomed_x = panned_x / zoom_level;
    // 4. Account for horizontal zoom (divide because it stretches time axis)
    let time_unzoomed_x = uniformly_unzoomed_x / zoom_level_x;

    // Now calculate fraction based on the base (unzoomed) graph width
    let base_graph_width = graph_width;
    let fraction = time_unzoomed_x / base_graph_width;

    let total_hours = fraction * 48.0; // 48 hours to support past-midnight
    let total_minutes = (total_hours * 60.0) as u32;

    // Calculate days, hours, and minutes
    let days = total_minutes / (24 * 60);
    let remaining_minutes = total_minutes % (24 * 60);
    let hours = remaining_minutes / 60;
    let minutes = remaining_minutes % 60;

    let target_date = BASE_DATE + chrono::Duration::days(days as i64);

    if let Some(new_time) = chrono::NaiveTime::from_hms_opt(hours, minutes, 0) {
        let new_datetime = target_date.and_time(new_time);
        set_time.set(new_datetime);
    }
}

fn render_graph(
    canvas: leptos::HtmlElement<leptos::html::Canvas>,
    stations: &[crate::models::StationNode],
    train_journeys: &[TrainJourney],
    current_time: chrono::NaiveDateTime,
    viewport: ViewportState,
    conflicts: &[Conflict],
    station_crossings: &[StationCrossing],
    graph: &RailwayGraph,
    show_station_crossings: bool,
    show_conflicts: bool,
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
    // Apply horizontal zoom to time axis by scaling hour_width
    zoomed_dimensions.hour_width *= viewport.zoom_level_x;

    // Draw grid and content in zoomed coordinate system
    time_labels::draw_hour_grid(&ctx, &zoomed_dimensions, viewport.zoom_level, viewport.pan_offset_x);
    graph_content::draw_station_grid(&ctx, &zoomed_dimensions, stations);
    graph_content::draw_double_track_indicators(&ctx, &zoomed_dimensions, stations, graph);

    // Draw train journeys
    let station_height = zoomed_dimensions.graph_height / stations.len() as f64;
    train_journeys::draw_train_journeys(
        &ctx,
        &zoomed_dimensions,
        stations,
        train_journeys,
        viewport.zoom_level,
        time_to_fraction,
    );

    // Draw conflicts if enabled
    if show_conflicts {
        conflict_indicators::draw_conflict_highlights(
            &ctx,
            &zoomed_dimensions,
            conflicts,
            station_height,
            viewport.zoom_level,
            time_to_fraction,
        );
    }

    // Draw station crossings if enabled
    if show_station_crossings {
        conflict_indicators::draw_station_crossings(
            &ctx,
            &zoomed_dimensions,
            station_crossings,
            station_height,
            viewport.zoom_level,
            time_to_fraction,
        );
    }

    // Draw current train positions
    train_positions::draw_current_train_positions(
        &ctx,
        &zoomed_dimensions,
        stations,
        train_journeys,
        station_height,
        current_time,
        viewport.zoom_level,
        time_to_fraction,
    );

    // Restore canvas context
    ctx.restore();

    // Draw labels at normal size but with adjusted positions for zoom/pan
    time_labels::draw_hour_labels(
        &ctx,
        &dimensions,
        viewport.zoom_level,
        viewport.zoom_level_x,
        viewport.pan_offset_x,
    );
    station_labels::draw_station_labels(
        &ctx,
        &dimensions,
        stations,
        viewport.zoom_level,
        viewport.pan_offset_y,
    );
    station_labels::draw_segment_toggles(
        &ctx,
        &dimensions,
        stations,
        graph,
        viewport.zoom_level,
        viewport.pan_offset_y,
    );

    // Draw time scrubber on top (adjusted for zoom/pan)
    time_scrubber::draw_time_scrubber(
        &ctx,
        &dimensions,
        current_time,
        viewport.zoom_level,
        viewport.pan_offset_x,
        time_to_fraction,
    );
}

fn clear_canvas(ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
    ctx.clear_rect(0.0, 0.0, width, height);
}

fn toggle_segment_double_track(
    clicked_segment: usize,
    stations: &[crate::models::StationNode],
    set_graph: WriteSignal<RailwayGraph>,
) {
    // segment index i represents the segment between stations[i-1] and stations[i]
    if clicked_segment == 0 || clicked_segment >= stations.len() {
        return;
    }

    let station1 = &stations[clicked_segment - 1];
    let station2 = &stations[clicked_segment];

    set_graph.update(move |graph| {
        // Get node indices for both stations
        let Some(node1) = graph.get_station_index(&station1.name) else {
            return;
        };
        let Some(node2) = graph.get_station_index(&station2.name) else {
            return;
        };

        // Find and toggle edges in both directions
        // Check node1 -> node2
        for edge in graph.graph.edge_indices() {
            if let Some((from, to)) = graph.graph.edge_endpoints(edge) {
                if (from == node1 && to == node2) || (from == node2 && to == node1) {
                    if let Some(weight) = graph.graph.edge_weight_mut(edge) {
                        weight.double_tracked = !weight.double_tracked;
                    }
                }
            }
        }
    });
}

fn handle_time_scrubbing(
    x: f64,
    canvas_width: f64,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    set_is_dragging: WriteSignal<bool>,
    set_visualization_time: WriteSignal<NaiveDateTime>,
) {
    let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;

    if x >= LEFT_MARGIN && x <= LEFT_MARGIN + graph_width {
        set_is_dragging.set(true);
        update_time_from_x(x, LEFT_MARGIN, graph_width, zoom_level, zoom_level_x, pan_offset_x, set_visualization_time);
    }
}
