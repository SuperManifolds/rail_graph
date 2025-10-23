use leptos::*;
use leptos::set_timeout_with_handle;
use std::time::Duration;
use std::rc::Rc;
use std::cell::Cell;
use chrono::NaiveDateTime;
use web_sys::{MouseEvent, WheelEvent, CanvasRenderingContext2d};
use wasm_bindgen::{JsCast, closure::Closure};
use crate::models::RailwayGraph;
use crate::conflict::Conflict;
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
    conflicts_memo: Memo<Vec<Conflict>>,
    show_conflicts: Signal<bool>,
    show_line_blocks: Signal<bool>,
    spacing_mode: Signal<crate::models::SpacingMode>,
    hovered_conflict: ReadSignal<Option<(Conflict, f64, f64)>>,
    hovered_journey_id: ReadSignal<Option<uuid::Uuid>>,
    display_stations: Signal<Vec<(petgraph::stable_graph::NodeIndex, crate::models::Node)>>,
    station_idx_map: leptos::Memo<std::collections::HashMap<usize, usize>>,
) {
    let (render_requested, set_render_requested) = create_signal(false);
    let is_disposed = Rc::new(Cell::new(false));
    let zoom_level = viewport.zoom_level;
    let zoom_level_x = viewport.zoom_level_x.expect("horizontal zoom enabled").0;
    let pan_offset_x = viewport.pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;

    {
        let is_disposed = Rc::clone(&is_disposed);
        on_cleanup(move || {
            is_disposed.set(true);
        });
    }

    create_effect(move |_| {
        // Track all dependencies
        let _ = train_journeys.get();
        let _ = visualization_time.get();
        let _ = graph.get();
        let _ = display_stations.get();
        let _ = station_idx_map.get();
        let _ = zoom_level.get();
        let _ = zoom_level_x.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();
        let _ = conflicts_memo.get();
        let _ = show_conflicts.get();
        let _ = hovered_conflict.get();
        let _ = show_line_blocks.get();
        let _ = hovered_journey_id.get();
        let _ = spacing_mode.get();

        if !render_requested.get_untracked() {
            set_render_requested.set(true);

            let window = web_sys::window().expect("window");
            let is_disposed = Rc::clone(&is_disposed);
            let callback = Closure::once(move || {
                // Check if component has been disposed
                if is_disposed.get() {
                    return;
                }

                // Check if component is still mounted before accessing signals
                let Some(canvas) = canvas_ref.get_untracked() else { return };

                set_render_requested.set(false);

                let journeys = train_journeys.get_untracked();
                let current = visualization_time.get_untracked();
                let current_graph = graph.get_untracked();
                let stations_for_render = display_stations.get_untracked();
                let idx_map = station_idx_map.get_untracked();

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
                let current_conflicts = conflicts_memo.get_untracked();
                let conflict_display = ConflictDisplayState {
                    conflicts: &current_conflicts,
                    show_conflicts: show_conflicts.get_untracked(),
                };
                let hovered = hovered_conflict.get_untracked();
                let hovered_journey_value = hovered_journey_id.get_untracked();
                let hover_state = HoverState {
                    hovered_conflict: hovered.as_ref().map(|(c, _, _)| c),
                    show_line_blocks: show_line_blocks.get_untracked(),
                    hovered_journey_id: hovered_journey_value.as_ref(),
                };
                let current_spacing_mode = spacing_mode.get_untracked();
                render_graph(&canvas, &stations_for_render, &journeys, current, &viewport, &conflict_display, &hover_state, &current_graph, &idx_map, current_spacing_mode);
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
    conflicts_memo: Memo<Vec<Conflict>>,
    display_stations: Signal<Vec<(petgraph::stable_graph::NodeIndex, crate::models::Node)>>,
    show_line_blocks: Signal<bool>,
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    set_hovered_conflict: WriteSignal<Option<(Conflict, f64, f64)>>,
    set_hovered_journey_id: WriteSignal<Option<uuid::Uuid>>,
    station_idx_map: leptos::Memo<std::collections::HashMap<usize, usize>>,
    graph: ReadSignal<RailwayGraph>,
    spacing_mode: Signal<crate::models::SpacingMode>,
) {
    let current_conflicts = conflicts_memo.get();
    let current_stations = display_stations.get();
    let idx_map = station_idx_map.get();
    let current_graph = graph.get();
    let current_spacing_mode = spacing_mode.get();

    // Calculate station positions for accurate hover detection
    let canvas_width = f64::from(canvas.width());
    let canvas_height = f64::from(canvas.height());
    let dimensions = GraphDimensions::new(canvas_width, canvas_height);
    let station_y_positions = current_graph.calculate_station_positions(
        &current_stations,
        current_spacing_mode,
        dimensions.graph_height,
        dimensions.top_margin,
    );

    let hovered = conflict_indicators::check_conflict_hover(
        x, y, &current_conflicts, &current_stations, &station_y_positions,
        canvas_width, canvas_height,
        viewport.zoom_level, viewport.zoom_level_x, viewport.pan_offset_x, viewport.pan_offset_y,
        &idx_map,
    );
    set_hovered_conflict.set(hovered);

    if show_line_blocks.get() {
        let journeys = train_journeys.get();
        let mut journeys_vec: Vec<_> = journeys.values().collect();
        journeys_vec.sort_by_key(|j| j.departure_time);
        let hovered_journey = train_journeys::check_journey_hover(
            x, y, &journeys_vec, &current_stations, &station_y_positions,
            canvas_width, canvas_height,
            &viewport
        );
        set_hovered_journey_id.set(hovered_journey);
    } else {
        set_hovered_journey_id.set(None);
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::too_many_lines)]
#[component]
#[must_use]
pub fn GraphCanvas(
    graph: ReadSignal<RailwayGraph>,
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    visualization_time: ReadSignal<NaiveDateTime>,
    set_visualization_time: WriteSignal<NaiveDateTime>,
    show_conflicts: Signal<bool>,
    show_line_blocks: Signal<bool>,
    spacing_mode: Signal<crate::models::SpacingMode>,
    hovered_journey_id: ReadSignal<Option<uuid::Uuid>>,
    set_hovered_journey_id: WriteSignal<Option<uuid::Uuid>>,
    conflicts_memo: Memo<Vec<Conflict>>,
    #[prop(optional)] pan_to_conflict_signal: Option<ReadSignal<Option<(f64, f64)>>>,
    display_stations: Signal<Vec<(petgraph::stable_graph::NodeIndex, crate::models::Node)>>,
    station_idx_map: leptos::Memo<std::collections::HashMap<usize, usize>>,
    initial_viewport: crate::models::ViewportState,
    on_viewport_change: leptos::Callback<crate::models::ViewportState>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (hovered_conflict, set_hovered_conflict) = create_signal(None::<(Conflict, f64, f64)>);
    let (space_pressed, set_space_pressed) = create_signal(false);

    // Track WASD keys for panning
    let (w_pressed, set_w_pressed) = create_signal(false);
    let (a_pressed, set_a_pressed) = create_signal(false);
    let (s_pressed, set_s_pressed) = create_signal(false);
    let (d_pressed, set_d_pressed) = create_signal(false);

    let viewport = canvas_viewport::create_viewport_signals(true);

    // Create a signal for canvas dimensions
    let canvas_dimensions = Signal::derive(move || {
        canvas_ref.get().map(|canvas| {
            let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
            (f64::from(canvas_elem.width()), f64::from(canvas_elem.height()))
        })
    });

    // Setup keyboard listeners for Space and WASD
    canvas_viewport::setup_keyboard_listeners(
        set_space_pressed,
        set_w_pressed,
        set_a_pressed,
        set_s_pressed,
        set_d_pressed,
        &viewport,
        canvas_dimensions,
        Some(1.0), // Min zoom of 1.0 for time graph
    );

    // Initialize viewport from saved state - only once on first mount
    let initialized = leptos::store_value(false);
    if !initialized.get_value() {
        viewport.set_zoom_level.set(initial_viewport.zoom_level);
        if let Some((_, set_zoom_x)) = viewport.zoom_level_x {
            set_zoom_x.set(initial_viewport.zoom_level_x.unwrap_or(1.0));
        }
        viewport.set_pan_offset_x.set(initial_viewport.pan_offset_x);
        viewport.set_pan_offset_y.set(initial_viewport.pan_offset_y);
        initialized.set_value(true);
    }
    let zoom_level = viewport.zoom_level;
    let set_zoom_level = viewport.set_zoom_level;
    let zoom_level_x = viewport.zoom_level_x.expect("horizontal zoom enabled").0;
    let pan_offset_x = viewport.pan_offset_x;
    let set_pan_offset_x = viewport.set_pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;
    let set_pan_offset_y = viewport.set_pan_offset_y;
    let is_panning = viewport.is_panning;

    // WASD continuous panning
    canvas_viewport::setup_wasd_panning(
        w_pressed, a_pressed, s_pressed, d_pressed,
        set_pan_offset_x, set_pan_offset_y,
        pan_offset_x, pan_offset_y,
    );

    // Save viewport changes to the view (debounced)
    let debounce_handle = store_value(None::<leptos::leptos_dom::helpers::TimeoutHandle>);

    create_effect(move |prev_state: Option<(f64, f64, f64, f64)>| {
        let zoom = zoom_level.get();
        let zoom_x = zoom_level_x.get();
        let pan_x = pan_offset_x.get();
        let pan_y = pan_offset_y.get();

        let current = (zoom, zoom_x, pan_x, pan_y);

        // Only update if values actually changed (skip initial render)
        let Some(prev) = prev_state else {
            return current;
        };

        if prev != current {
            // Clear existing timer
            debounce_handle.update_value(|handle| {
                if let Some(h) = handle.take() {
                    h.clear();
                }
            });

            // Set new timer to save after 300ms of no changes
            let handle = set_timeout_with_handle(
                move || {
                    on_viewport_change.call(crate::models::ViewportState {
                        zoom_level: zoom,
                        zoom_level_x: Some(zoom_x),
                        pan_offset_x: pan_x,
                        pan_offset_y: pan_y,
                    });
                },
                Duration::from_millis(300)
            ).ok();

            debounce_handle.set_value(handle);
        }

        current
    });

    if let Some(pan_signal) = pan_to_conflict_signal {
        create_effect(move |_| {
            if let Some((time_fraction, station_pos)) = pan_signal.get() {
                if let Some(canvas_elem) = canvas_ref.get() {
                    let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
                    let canvas_width = f64::from(canvas.width());
                    let canvas_height = f64::from(canvas.height());

                    let dims = GraphDimensions::new(canvas_width, canvas_height);

                    let current_graph = graph.get();
                    let current_stations = display_stations.get();
                    let current_spacing_mode = spacing_mode.get();

                    // Calculate station positions to get accurate Y coordinate
                    let station_y_positions = current_graph.calculate_station_positions(
                        &current_stations,
                        current_spacing_mode,
                        dims.graph_height,
                        dims.top_margin,
                    );

                    let target_zoom = 8.0;
                    set_zoom_level.set(target_zoom);

                    // Calculate Y position using actual station positions
                    #[allow(clippy::excessive_nesting)]
                    let calculate_y_pos = |pos: f64, positions: &[f64]| -> f64 {
                        if pos.fract() < f64::EPSILON {
                            // Integer position - use direct lookup
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            let idx = pos as usize;
                            positions.get(idx).copied().unwrap_or(0.0)
                        } else {
                            // Fractional position - interpolate between two stations
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            let idx1 = pos.floor() as usize;
                            let idx2 = pos.ceil() as usize;
                            let fraction = pos.fract();

                            let y1 = positions.get(idx1).copied().unwrap_or(0.0);
                            let y2 = positions.get(idx2).copied().unwrap_or(y1);
                            y1 + (fraction * (y2 - y1))
                        }
                    };

                    let y_pos = calculate_y_pos(station_pos, &station_y_positions);

                    let target_x = (time_fraction * dims.hour_width * target_zoom * target_zoom) - (canvas_width / 2.0);
                    // Subtract TOP_MARGIN since station_y_positions include it but we're in transformed coords
                    let target_y = ((y_pos - TOP_MARGIN) * target_zoom) - (canvas_height / 2.0);

                    set_pan_offset_x.set(-target_x);
                    set_pan_offset_y.set(-target_y);
                }
            }
        });
    }

    setup_render_effect(
        canvas_ref, train_journeys, visualization_time, graph, &viewport,
        conflicts_memo, show_conflicts, show_line_blocks, spacing_mode,
        hovered_conflict, hovered_journey_id, display_stations, station_idx_map
    );

    let handle_mouse_down = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = f64::from(ev.client_x()) - rect.left();

            // Only handle time scrubbing if space is not pressed
            if !space_pressed.get() {
                let canvas_width = f64::from(canvas.width());
                handle_time_scrubbing(x, canvas_width, zoom_level.get(), zoom_level_x.get(), pan_offset_x.get(), set_is_dragging, set_visualization_time);
            }
        }
    };

    let handle_mouse_move = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = f64::from(ev.client_x()) - rect.left();
            let y = f64::from(ev.client_y()) - rect.top();

            // If space is pressed and not yet panning, start panning
            if space_pressed.get() && !is_panning.get() {
                canvas_viewport::handle_pan_start(x, y, &viewport);
            }

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
                handle_mouse_move_hover(x, y, canvas, viewport_state, conflicts_memo, display_stations, show_line_blocks, train_journeys, set_hovered_conflict, set_hovered_journey_id, station_idx_map, graph, spacing_mode);
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

            // Check if cursor is over time labels section (top margin area)
            let over_time_labels = mouse_y < TOP_MARGIN && mouse_x >= LEFT_MARGIN;

            if over_time_labels || (mouse_x >= LEFT_MARGIN && mouse_x <= LEFT_MARGIN + graph_width &&
               mouse_y >= TOP_MARGIN && mouse_y <= TOP_MARGIN + graph_height) {

                let graph_mouse_x = mouse_x - LEFT_MARGIN;
                let graph_mouse_y = mouse_y - TOP_MARGIN;

                // Minimum zoom matches the default viewport zoom level of 1.0
                // At this zoom, stations are positioned to fit the screen perfectly
                let min_zoom = Some(1.0);

                canvas_viewport::handle_zoom(&ev, graph_mouse_x, graph_mouse_y, &viewport, min_zoom, Some((graph_width, graph_height)), over_time_labels);
            }
        }
    };

    let cursor_style = move || {
        if is_panning.get() {
            "cursor: grabbing;"
        } else if space_pressed.get() {
            "cursor: grab;"
        } else {
            "cursor: crosshair;"
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
                style=cursor_style
            ></canvas>

            <ConflictTooltip hovered_conflict=hovered_conflict display_nodes=display_stations />
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

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn render_graph(
    canvas: &leptos::HtmlElement<leptos::html::Canvas>,
    stations: &[(petgraph::stable_graph::NodeIndex, crate::models::Node)],
    train_journeys: &std::collections::HashMap<uuid::Uuid, TrainJourney>,
    current_time: chrono::NaiveDateTime,
    viewport: &ViewportState,
    conflict_display: &ConflictDisplayState,
    hover_state: &HoverState,
    graph: &RailwayGraph,
    station_idx_map: &std::collections::HashMap<usize, usize>,
    spacing_mode: crate::models::SpacingMode,
) {
    let canvas_element: &web_sys::HtmlCanvasElement = canvas;
    let canvas_width = f64::from(canvas_element.width());
    let canvas_height = f64::from(canvas_element.height());

    // Create dimensions once for the entire render
    let dimensions = GraphDimensions::new(canvas_width, canvas_height);

    // Calculate station Y positions based on spacing mode
    let station_y_positions = graph.calculate_station_positions(
        stations,
        spacing_mode,
        dimensions.graph_height,
        dimensions.top_margin,
    );

    // Filter journeys to only those visible in viewport (avoid cloning off-screen journeys)
    let visible_hour_width = viewport.zoom_level * viewport.zoom_level_x * dimensions.hour_width;
    let visible_start = -viewport.pan_offset_x / visible_hour_width;
    let visible_end = visible_start + (dimensions.graph_width / visible_hour_width);

    let mut journeys_vec: Vec<&TrainJourney> = train_journeys.values()
        .filter(|journey| {
            // Quick time-based culling: check if journey overlaps visible time range
            if let (Some((_, start, _)), Some((_, _, end))) =
                (journey.station_times.first(), journey.station_times.last()) {
                let start_frac = time_to_fraction(*start);
                let end_frac = time_to_fraction(*end);

                // Journey is visible if it overlaps with visible range
                end_frac >= visible_start && start_frac <= visible_end
            } else {
                false
            }
        })
        .collect();

    // Sort by departure time for consistent draw order (prevents z-fighting)
    journeys_vec.sort_by_key(|j| j.departure_time);

    let Ok(Some(context)) = canvas_element.get_context("2d") else {
        leptos::logging::warn!("Failed to get 2D context");
        return;
    };

    let Ok(ctx) = context.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        leptos::logging::warn!("Failed to cast to 2D rendering context");
        return;
    };

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
    graph_content::draw_station_grid(&ctx, &zoomed_dimensions, stations, &station_y_positions, viewport.zoom_level, viewport.pan_offset_x);
    graph_content::draw_double_track_indicators(&ctx, &zoomed_dimensions, stations, &station_y_positions, graph, viewport.zoom_level, viewport.pan_offset_x);

    // Draw train journeys
    train_journeys::draw_train_journeys(
        &ctx,
        &zoomed_dimensions,
        stations,
        &station_y_positions,
        &journeys_vec,
        viewport.zoom_level,
        time_to_fraction,
    );

    // Draw conflicts if enabled
    if conflict_display.show_conflicts {
        // Filter conflicts to only visible ones
        let visible_conflicts: Vec<&Conflict> = conflict_display.conflicts
            .iter()
            .filter(|conflict| {
                let time_frac = time_to_fraction(conflict.time);
                time_frac >= visible_start && time_frac <= visible_end
            })
            .collect();

        conflict_indicators::draw_conflict_highlights(
            &ctx,
            &zoomed_dimensions,
            &visible_conflicts,
            &station_y_positions,
            viewport.zoom_level,
            time_to_fraction,
            station_idx_map,
        );

        // Draw block visualization for hovered conflicts (BlockViolation, HeadOn, Overtaking)
        if let Some(conflict) = hover_state.hovered_conflict {
            // Show blocks for any conflict type that has segment timing information
            if conflict.segment1_times.is_some() && conflict.segment2_times.is_some() {
                conflict_indicators::draw_block_violation_visualization(
                    &ctx,
                    &zoomed_dimensions,
                    conflict,
                    &journeys_vec,
                    &station_y_positions,
                    viewport.zoom_level,
                    time_to_fraction,
                    station_idx_map,
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
                    &station_y_positions,
                    viewport.zoom_level,
                    time_to_fraction,
                );
            }
        }
    }

    // Draw current train positions
    train_positions::draw_current_train_positions(
        &ctx,
        &zoomed_dimensions,
        stations,
        &journeys_vec,
        &station_y_positions,
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
        &station_y_positions,
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
