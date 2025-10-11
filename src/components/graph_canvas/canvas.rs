use leptos::*;
use chrono::NaiveDateTime;
use web_sys::{MouseEvent, WheelEvent, CanvasRenderingContext2d};
use wasm_bindgen::{JsCast, closure::Closure};
use crate::models::{RailwayGraph, Stations, Tracks};
use crate::conflict::{Conflict, StationCrossing};
use crate::train_journey::TrainJourney;
use crate::components::conflict_tooltip::ConflictTooltip;
use crate::components::canvas_viewport;
use crate::constants::BASE_DATE;
use crate::time::time_to_fraction;
use super::{station_labels, time_labels, conflict_indicators, train_positions, train_journeys, time_scrubber, graph_content};
use super::types::{GraphDimensions, ViewportState, ConflictDisplayState, HoverState};

// Layout constants for the graph canvas
pub const LEFT_MARGIN: f64 = 120.0;
pub const TOP_MARGIN: f64 = 60.0;
pub const RIGHT_PADDING: f64 = 20.0;
pub const BOTTOM_PADDING: f64 = 20.0;

#[allow(clippy::too_many_arguments)]
fn setup_render_effect(
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    visualization_time: ReadSignal<NaiveDateTime>,
    graph: ReadSignal<RailwayGraph>,
    viewport: &canvas_viewport::ViewportSignals,
    conflicts_and_crossings: Memo<(Vec<Conflict>, Vec<StationCrossing>)>,
    show_station_crossings: Signal<bool>,
    show_conflicts: Signal<bool>,
    show_line_blocks: Signal<bool>,
    hovered_conflict: ReadSignal<Option<(Conflict, f64, f64)>>,
    hovered_journey_id: ReadSignal<Option<uuid::Uuid>>,
) {
    let (render_requested, set_render_requested) = create_signal(false);
    let zoom_level = viewport.zoom_level;
    let zoom_level_x = viewport.zoom_level_x.expect("horizontal zoom enabled").0;
    let pan_offset_x = viewport.pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;

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
        let _ = hovered_conflict.get();
        let _ = show_line_blocks.get();
        let _ = hovered_journey_id.get();

        if !render_requested.get_untracked() {
            set_render_requested.set(true);

            let window = web_sys::window().expect("window");
            let callback = Closure::once(move || {
                set_render_requested.set(false);

                let journeys = train_journeys.get_untracked();
                let current = visualization_time.get_untracked();
                let current_graph = graph.get_untracked();
                let stations_for_render = current_graph.get_all_stations_ordered();

                let Some(canvas) = canvas_ref.get_untracked() else { return };

                let zoom = zoom_level.get_untracked();
                let zoom_x = zoom_level_x.get_untracked();
                let pan_x = pan_offset_x.get_untracked();
                let pan_y = pan_offset_y.get_untracked();

                let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
                // Browser dimensions are always non-negative
                #[allow(clippy::cast_sign_loss)]
                let container_width = canvas_elem.client_width() as u32;
                #[allow(clippy::cast_sign_loss)]
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
                let conflict_display = ConflictDisplayState {
                    conflicts: &current_conflicts,
                    station_crossings: &current_station_crossings,
                    show_conflicts: show_conflicts.get_untracked(),
                    show_station_crossings: show_station_crossings.get_untracked(),
                };
                let hovered = hovered_conflict.get_untracked();
                let hovered_journey_value = hovered_journey_id.get_untracked();
                let hover_state = HoverState {
                    hovered_conflict: hovered.as_ref().map(|(c, _, _)| c),
                    show_line_blocks: show_line_blocks.get_untracked(),
                    hovered_journey_id: hovered_journey_value.as_ref(),
                };
                render_graph(&canvas, &stations_for_render, &journeys, current, &viewport, &conflict_display, &hover_state, &current_graph);
            });

            let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
            callback.forget();
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn handle_mouse_move_hover(
    x: f64,
    y: f64,
    canvas: &web_sys::HtmlCanvasElement,
    viewport: ViewportState,
    conflicts_and_crossings: Memo<(Vec<Conflict>, Vec<StationCrossing>)>,
    graph: ReadSignal<RailwayGraph>,
    show_line_blocks: Signal<bool>,
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    set_hovered_conflict: WriteSignal<Option<(Conflict, f64, f64)>>,
    set_hovered_journey_id: WriteSignal<Option<uuid::Uuid>>,
) {
    let (current_conflicts, _) = conflicts_and_crossings.get();
    let current_graph = graph.get();
    let current_stations = current_graph.get_all_stations_ordered();
    let hovered = conflict_indicators::check_conflict_hover(
        x, y, &current_conflicts, &current_stations,
        f64::from(canvas.width()), f64::from(canvas.height()),
        viewport.zoom_level, viewport.zoom_level_x, viewport.pan_offset_x, viewport.pan_offset_y
    );
    set_hovered_conflict.set(hovered);

    if show_line_blocks.get() {
        let journeys = train_journeys.get();
        let journeys_vec: Vec<_> = journeys.values().collect();
        let hovered_journey = train_journeys::check_journey_hover(
            x, y, &journeys_vec, &current_stations,
            f64::from(canvas.width()), f64::from(canvas.height()),
            &viewport
        );
        set_hovered_journey_id.set(hovered_journey);
    } else {
        set_hovered_journey_id.set(None);
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[component]
#[must_use]
pub fn GraphCanvas(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_lines: WriteSignal<Vec<crate::models::Line>>,
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    visualization_time: ReadSignal<NaiveDateTime>,
    set_visualization_time: WriteSignal<NaiveDateTime>,
    show_station_crossings: Signal<bool>,
    show_conflicts: Signal<bool>,
    show_line_blocks: Signal<bool>,
    hovered_journey_id: ReadSignal<Option<uuid::Uuid>>,
    set_hovered_journey_id: WriteSignal<Option<uuid::Uuid>>,
    conflicts_and_crossings: Memo<(Vec<Conflict>, Vec<StationCrossing>)>,
    #[prop(optional)] pan_to_conflict_signal: Option<ReadSignal<Option<(f64, f64)>>>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (hovered_conflict, set_hovered_conflict) = create_signal(None::<(Conflict, f64, f64)>);

    let viewport = canvas_viewport::create_viewport_signals(true);
    let zoom_level = viewport.zoom_level;
    let set_zoom_level = viewport.set_zoom_level;
    let (zoom_level_x, set_zoom_level_x) = viewport.zoom_level_x.expect("horizontal zoom enabled");
    let pan_offset_x = viewport.pan_offset_x;
    let set_pan_offset_x = viewport.set_pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;
    let set_pan_offset_y = viewport.set_pan_offset_y;
    let is_panning = viewport.is_panning;

    if let Some(pan_signal) = pan_to_conflict_signal {
        create_effect(move |_| {
            if let Some((time_fraction, station_pos)) = pan_signal.get() {
                if let Some(canvas_elem) = canvas_ref.get() {
                    let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
                    let canvas_width = f64::from(canvas.width());
                    let canvas_height = f64::from(canvas.height());

                    let dims = GraphDimensions::new(canvas_width, canvas_height);

                    let current_graph = graph.get();
                    let station_count = current_graph.get_all_stations_ordered().len() as f64;

                    let target_zoom = 4.0;
                    set_zoom_level.set(target_zoom);
                    set_zoom_level_x.set(target_zoom);

                    let target_x = (time_fraction * dims.hour_width * target_zoom * target_zoom) - (canvas_width / 2.0);
                    let target_y = (station_pos * (dims.graph_height / station_count.max(1.0)) * target_zoom) - (canvas_height / 2.0);

                    set_pan_offset_x.set(-target_x);
                    set_pan_offset_y.set(-target_y);
                }
            }
        });
    }

    setup_render_effect(
        canvas_ref, train_journeys, visualization_time, graph, &viewport,
        conflicts_and_crossings, show_station_crossings, show_conflicts, show_line_blocks,
        hovered_conflict, hovered_journey_id
    );

    let handle_mouse_down = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = f64::from(ev.client_x()) - rect.left();
            let y = f64::from(ev.client_y()) - rect.top();

            if ev.button() == 2 || ev.ctrl_key() {
                canvas_viewport::handle_pan_start(x, y, &viewport);
            } else {
                let canvas_width = f64::from(canvas.width());
                let canvas_height = f64::from(canvas.height());
                let current_graph = graph.get();
                let current_stations = current_graph.get_all_stations_ordered();

                if let Some(clicked_segment) = station_labels::check_toggle_click(
                    x, y, canvas_height, &current_stations,
                    zoom_level.get(), pan_offset_y.get()
                ) {
                    toggle_segment_double_track(clicked_segment, &current_stations, set_graph, set_lines);
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
            let x = f64::from(ev.client_x()) - rect.left();
            let y = f64::from(ev.client_y()) - rect.top();

            if is_panning.get() {
                canvas_viewport::handle_pan_move(x, y, &viewport);
            } else if is_dragging.get() {
                let canvas_width = f64::from(canvas.width());
                let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;

                if x >= LEFT_MARGIN && x <= LEFT_MARGIN + graph_width {
                    update_time_from_x(x, LEFT_MARGIN, graph_width, zoom_level.get(), zoom_level_x.get(), pan_offset_x.get(), set_visualization_time);
                }
            } else {
                let viewport_state = ViewportState {
                    zoom_level: zoom_level.get(),
                    zoom_level_x: zoom_level_x.get(),
                    pan_offset_x: pan_offset_x.get(),
                    pan_offset_y: pan_offset_y.get(),
                };
                handle_mouse_move_hover(x, y, canvas, viewport_state, conflicts_and_crossings, graph, show_line_blocks, train_journeys, set_hovered_conflict, set_hovered_journey_id);
            }
        }
    };

    let handle_mouse_up = move |_ev: MouseEvent| {
        set_is_dragging.set(false);
        canvas_viewport::handle_pan_end(&viewport);
    };

    let handle_mouse_leave = move |_ev: MouseEvent| {
        set_is_dragging.set(false);
        canvas_viewport::handle_pan_end(&viewport);
        set_hovered_conflict.set(None);
    };

    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = f64::from(ev.client_x()) - rect.left();
            let mouse_y = f64::from(ev.client_y()) - rect.top();

            let canvas_width = f64::from(canvas.width());
            let canvas_height = f64::from(canvas.height());
            let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;
            let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;

            if mouse_x >= LEFT_MARGIN && mouse_x <= LEFT_MARGIN + graph_width &&
               mouse_y >= TOP_MARGIN && mouse_y <= TOP_MARGIN + graph_height {

                let graph_mouse_x = mouse_x - LEFT_MARGIN;
                let graph_mouse_y = mouse_y - TOP_MARGIN;

                canvas_viewport::handle_zoom(&ev, graph_mouse_x, graph_mouse_y, &viewport);
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

            <ConflictTooltip hovered_conflict=hovered_conflict stations=Signal::derive(move || graph.get().get_all_stations_ordered()) />
        </div>
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

    let target_date = BASE_DATE + chrono::Duration::days(i64::from(days));

    if let Some(new_time) = chrono::NaiveTime::from_hms_opt(hours, minutes, 0) {
        let new_datetime = target_date.and_time(new_time);
        set_time.set(new_datetime);
    }
}

fn render_graph(
    canvas: &leptos::HtmlElement<leptos::html::Canvas>,
    stations: &[crate::models::StationNode],
    train_journeys: &std::collections::HashMap<uuid::Uuid, TrainJourney>,
    current_time: chrono::NaiveDateTime,
    viewport: &ViewportState,
    conflict_display: &ConflictDisplayState,
    hover_state: &HoverState,
    graph: &RailwayGraph,
) {
    let canvas_element: &web_sys::HtmlCanvasElement = canvas;
    let canvas_width = f64::from(canvas_element.width());
    let canvas_height = f64::from(canvas_element.height());

    // Convert HashMap to Vec for drawing functions
    let journeys_vec: Vec<_> = train_journeys.values().cloned().collect();

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
    time_labels::draw_hour_grid(&ctx, &zoomed_dimensions, viewport.zoom_level, viewport.zoom_level_x, viewport.pan_offset_x);
    graph_content::draw_station_grid(&ctx, &zoomed_dimensions, stations, viewport.zoom_level, viewport.pan_offset_x);
    graph_content::draw_double_track_indicators(&ctx, &zoomed_dimensions, stations, graph, viewport.zoom_level, viewport.pan_offset_x);

    // Draw train journeys
    #[allow(clippy::cast_precision_loss)]
    let station_height = zoomed_dimensions.graph_height / (stations.len() as f64);
    train_journeys::draw_train_journeys(
        &ctx,
        &zoomed_dimensions,
        stations,
        &journeys_vec,
        viewport.zoom_level,
        time_to_fraction,
    );

    // Draw conflicts if enabled
    if conflict_display.show_conflicts {
        conflict_indicators::draw_conflict_highlights(
            &ctx,
            &zoomed_dimensions,
            conflict_display.conflicts,
            station_height,
            viewport.zoom_level,
            time_to_fraction,
        );

        // Draw block visualization for hovered block violation
        if let Some(conflict) = hover_state.hovered_conflict {
            if conflict.conflict_type == crate::conflict::ConflictType::BlockViolation {
                conflict_indicators::draw_block_violation_visualization(
                    &ctx,
                    &zoomed_dimensions,
                    conflict,
                    &journeys_vec,
                    station_height,
                    viewport.zoom_level,
                    time_to_fraction,
                );
            }
        }
    }

    // Draw journey blocks if enabled and hovering over a journey
    if hover_state.show_line_blocks {
        if let Some(journey_id) = hover_state.hovered_journey_id {
            if let Some(journey) = train_journeys.get(journey_id) {
                conflict_indicators::draw_journey_blocks(
                    &ctx,
                    &zoomed_dimensions,
                    journey,
                    stations,
                    station_height,
                    viewport.zoom_level,
                    time_to_fraction,
                );
            }
        }
    }

    // Draw station crossings if enabled
    if conflict_display.show_station_crossings {
        conflict_indicators::draw_station_crossings(
            &ctx,
            &zoomed_dimensions,
            conflict_display.station_crossings,
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
        &journeys_vec,
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
        graph,
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
        viewport.zoom_level_x,
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
    set_lines: WriteSignal<Vec<crate::models::Line>>,
) {
    // segment index i represents the segment between stations[i-1] and stations[i]
    if clicked_segment == 0 || clicked_segment >= stations.len() {
        return;
    }

    let station1_name = stations[clicked_segment - 1].name.clone();
    let station2_name = stations[clicked_segment].name.clone();

    // Toggle track in the graph model
    set_graph.update(|g| {
        let changed_edges = g.toggle_segment_double_track(&station1_name, &station2_name);

        // Update lines to fix incompatible track assignments
        set_lines.update(|current_lines| {
            for (edge_index, new_track_count) in changed_edges {
                for line in current_lines.iter_mut() {
                    line.fix_track_indices_after_change(edge_index, new_track_count, g);
                }
            }
        });
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
