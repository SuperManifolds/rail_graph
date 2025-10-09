use crate::models::{RailwayGraph, Line, Track};
use crate::components::infrastructure_canvas::{auto_layout, renderer, hit_detection};
use crate::components::canvas_viewport;
use crate::components::add_station::AddStation;
use crate::components::delete_station_confirmation::DeleteStationConfirmation;
use crate::components::edit_station::EditStation;
use crate::components::edit_track::EditTrack;
use leptos::{wasm_bindgen, web_sys, component, view, ReadSignal, WriteSignal, IntoView, create_node_ref, create_signal, create_effect, SignalGet, SignalSet, SignalGetUntracked};
use petgraph::graph::{NodeIndex, EdgeIndex};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, MouseEvent, WheelEvent};

#[derive(Clone, Copy, PartialEq)]
enum EditMode {
    None,
    AddingTrack,
}

fn handle_mouse_down_adding_track(
    clicked_station: NodeIndex,
    selected_station: ReadSignal<Option<NodeIndex>>,
    set_selected_station: WriteSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) {
    let Some(first_station) = selected_station.get() else {
        set_selected_station.set(Some(clicked_station));
        return;
    };

    if first_station != clicked_station {
        let mut updated_graph = graph.get();
        use crate::models::{Track, TrackDirection};
        updated_graph.add_track(first_station, clicked_station, vec![Track { direction: TrackDirection::Bidirectional }]);
        set_graph.set(updated_graph);
    }
    set_selected_station.set(None);
}

fn handle_mouse_move_hover_detection(
    x: f64,
    y: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    graph: ReadSignal<RailwayGraph>,
    editing_station: ReadSignal<Option<NodeIndex>>,
    set_is_over_station: WriteSignal<bool>,
    set_is_over_edited_station: WriteSignal<bool>,
    set_is_over_track: WriteSignal<bool>,
) {
    let world_x = (x - pan_x) / zoom;
    let world_y = (y - pan_y) / zoom;

    let current_graph = graph.get();
    if let Some(hovered_station) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) {
        let is_editing_this = Some(hovered_station) == editing_station.get();
        set_is_over_station.set(true);
        set_is_over_edited_station.set(is_editing_this);
        set_is_over_track.set(false);
    } else if hit_detection::find_track_at_position(&current_graph, world_x, world_y).is_some() {
        set_is_over_station.set(false);
        set_is_over_edited_station.set(false);
        set_is_over_track.set(true);
    } else {
        set_is_over_station.set(false);
        set_is_over_edited_station.set(false);
        set_is_over_track.set(false);
    }
}

fn screen_to_world(screen_x: f64, screen_y: f64, zoom: f64, pan_x: f64, pan_y: f64) -> (f64, f64) {
    ((screen_x - pan_x) / zoom, (screen_y - pan_y) / zoom)
}

fn add_station_handler(
    name: String,
    passing_loop: bool,
    connect_to: Option<NodeIndex>,
    platforms: Vec<crate::models::Platform>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_show_add_station: WriteSignal<bool>,
) {
    let mut current_graph = graph.get();
    let node_idx = current_graph.add_or_get_station(name.clone());

    if let Some(node) = current_graph.graph.node_weight_mut(node_idx) {
        node.passing_loop = passing_loop;
        node.platforms = platforms;
    }

    if let Some(connect_idx) = connect_to {
        if let Some(connect_pos) = current_graph.get_station_position(connect_idx) {
            current_graph.set_station_position(node_idx, (connect_pos.0 + 80.0, connect_pos.1 + 40.0));
        }
        use crate::models::{Track, TrackDirection};
        current_graph.add_track(connect_idx, node_idx, vec![Track { direction: TrackDirection::Bidirectional }]);
    }

    set_graph.set(current_graph);
    set_show_add_station.set(false);
}

fn edit_station_handler(
    station_idx: NodeIndex,
    new_name: String,
    passing_loop: bool,
    platforms: Vec<crate::models::Platform>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
) {
    let mut current_graph = graph.get();

    if let Some(node) = current_graph.graph.node_weight_mut(station_idx) {
        let old_name = node.name.clone();
        node.name = new_name.clone();
        node.passing_loop = passing_loop;
        node.platforms = platforms;

        current_graph.station_name_to_index.remove(&old_name);
        current_graph.station_name_to_index.insert(new_name, station_idx);
    }

    set_graph.set(current_graph);
    set_editing_station.set(None);
}

fn delete_station_handler(
    station_idx: NodeIndex,
    graph: ReadSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_delete_affected_lines: WriteSignal<Vec<String>>,
    set_station_to_delete: WriteSignal<Option<NodeIndex>>,
    set_delete_station_name: WriteSignal<String>,
    set_show_delete_confirmation: WriteSignal<bool>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
) {
    let current_graph = graph.get();
    let current_lines = lines.get();

    let station_edges = current_graph.get_station_edges(station_idx);

    let affected: Vec<String> = current_lines
        .iter()
        .filter(|line| line.uses_any_edge(&station_edges))
        .map(|line| line.id.clone())
        .collect();

    set_delete_affected_lines.set(affected);
    set_station_to_delete.set(Some(station_idx));
    if let Some(name) = current_graph.get_station_name(station_idx) {
        set_delete_station_name.set(name.to_string());
    }
    set_show_delete_confirmation.set(true);
    set_editing_station.set(None);
}

fn confirm_delete_station_handler(
    station_to_delete: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_show_delete_confirmation: WriteSignal<bool>,
    set_station_to_delete: WriteSignal<Option<NodeIndex>>,
) {
    let Some(station_idx) = station_to_delete.get() else { return };

    let mut current_graph = graph.get();
    let mut current_lines = lines.get();

    let (removed_edges, bypass_mapping) = current_graph.delete_station(station_idx);

    for line in &mut current_lines {
        line.update_route_after_deletion(&removed_edges, &bypass_mapping);
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_show_delete_confirmation.set(false);
    set_station_to_delete.set(None);
}

fn edit_track_handler(
    edge_idx: EdgeIndex,
    new_tracks: Vec<Track>,
    new_distance: Option<f64>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
) {
    let mut current_graph = graph.get();
    let mut current_lines = lines.get();
    let edge_index = edge_idx.index();
    let new_track_count = new_tracks.len();

    if let Some(track_segment) = current_graph.graph.edge_weight_mut(edge_idx) {
        track_segment.tracks = new_tracks;
        track_segment.distance = new_distance;
    }

    for line in &mut current_lines {
        line.fix_track_indices_after_change(edge_index, new_track_count, &current_graph);
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_editing_track.set(None);
}

fn delete_track_handler(
    edge_idx: EdgeIndex,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
) {
    let mut current_graph = graph.get();
    let mut current_lines = lines.get();

    current_graph.graph.remove_edge(edge_idx);

    let edge_index = edge_idx.index();
    for line in &mut current_lines {
        line.forward_route.retain(|segment| segment.edge_index != edge_index);
        line.return_route.retain(|segment| segment.edge_index != edge_index);
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_editing_track.set(None);
}

fn get_canvas_cursor_style(
    dragging_station: ReadSignal<Option<NodeIndex>>,
    edit_mode: ReadSignal<EditMode>,
    is_over_edited_station: ReadSignal<bool>,
    is_over_station: ReadSignal<bool>,
    is_over_track: ReadSignal<bool>,
) -> &'static str {
    if dragging_station.get().is_some() {
        "cursor: grabbing;"
    } else {
        match edit_mode.get() {
            EditMode::AddingTrack => "cursor: pointer;",
            EditMode::None => {
                if is_over_edited_station.get() {
                    "cursor: grab;"
                } else if is_over_station.get() || is_over_track.get() {
                    "cursor: pointer;"
                } else {
                    "cursor: grab;"
                }
            }
        }
    }
}

fn setup_auto_layout_effect(
    auto_layout_enabled: ReadSignal<bool>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
) {
    create_effect(move |_| {
        if !auto_layout_enabled.get() {
            return;
        }

        let mut current_graph = graph.get();

        let has_unpositioned = current_graph
            .graph
            .node_indices()
            .any(|idx| current_graph.get_station_position(idx).is_none());

        if has_unpositioned && current_graph.graph.node_count() > 0 {
            let Some(canvas) = canvas_ref.get() else { return };
            let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
            let height = f64::from(canvas_elem.client_height());
            auto_layout::apply_layout(&mut current_graph, height);
            set_graph.set(current_graph);
        }
    });
}

fn setup_render_effect(
    graph: ReadSignal<RailwayGraph>,
    zoom_level: ReadSignal<f64>,
    pan_offset_x: ReadSignal<f64>,
    pan_offset_y: ReadSignal<f64>,
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
) {
    create_effect(move |_| {
        let current_graph = graph.get();
        let _ = zoom_level.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();

        let Some(canvas) = canvas_ref.get() else { return };

        let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
        let container_width = canvas_elem.client_width() as u32;
        let container_height = canvas_elem.client_height() as u32;

        if container_width > 0 && container_height > 0 {
            canvas_elem.set_width(container_width);
            canvas_elem.set_height(container_height);
        }

        let Some(ctx) = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|ctx| ctx.dyn_into::<CanvasRenderingContext2d>().ok())
        else {
            return;
        };

        let zoom = zoom_level.get_untracked();
        let pan_x = pan_offset_x.get_untracked();
        let pan_y = pan_offset_y.get_untracked();

        renderer::draw_infrastructure(&ctx, &current_graph, (f64::from(container_width), f64::from(container_height)), zoom, pan_x, pan_y);
    });
}

fn create_event_handlers(
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
    edit_mode: ReadSignal<EditMode>,
    selected_station: ReadSignal<Option<NodeIndex>>,
    set_selected_station: WriteSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    editing_station: ReadSignal<Option<NodeIndex>>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
    dragging_station: ReadSignal<Option<NodeIndex>>,
    set_dragging_station: WriteSignal<Option<NodeIndex>>,
    set_is_over_station: WriteSignal<bool>,
    set_is_over_edited_station: WriteSignal<bool>,
    set_is_over_track: WriteSignal<bool>,
    auto_layout_enabled: ReadSignal<bool>,
    viewport: &canvas_viewport::ViewportSignals,
) -> (impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(WheelEvent)) {
    let zoom_level = viewport.zoom_level;
    let pan_offset_x = viewport.pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;
    let is_panning = viewport.is_panning;
    let viewport_copy = *viewport;

    let handle_mouse_down = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let screen_x = f64::from(ev.client_x()) - rect.left();
            let screen_y = f64::from(ev.client_y()) - rect.top();

            let current_mode = edit_mode.get();
            let zoom = zoom_level.get();
            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();
            let (world_x, world_y) = screen_to_world(screen_x, screen_y, zoom, pan_x, pan_y);

            match current_mode {
                EditMode::AddingTrack => {
                    let current_graph = graph.get();
                    let Some(clicked_station) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) else {
                        return;
                    };
                    handle_mouse_down_adding_track(clicked_station, selected_station, set_selected_station, graph, set_graph);
                }
                EditMode::None => {
                    let current_graph = graph.get();
                    match hit_detection::find_station_at_position(&current_graph, world_x, world_y) {
                        Some(clicked_station) if Some(clicked_station) == editing_station.get() => {
                            set_dragging_station.set(Some(clicked_station));
                        }
                        None if ev.button() == 2 || ev.ctrl_key() || ev.button() == 0 => {
                            canvas_viewport::handle_pan_start(screen_x, screen_y, &viewport_copy);
                        }
                        _ => {}
                    }
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
                canvas_viewport::handle_pan_move(x, y, &viewport_copy);
            } else if let Some(station_idx) = dragging_station.get() {
                let zoom = zoom_level.get();
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();
                let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);

                let mut current_graph = graph.get();
                current_graph.set_station_position(station_idx, (world_x, world_y));
                set_graph.set(current_graph);
            } else {
                handle_mouse_move_hover_detection(
                    x, y, zoom_level.get(), pan_offset_x.get(), pan_offset_y.get(),
                    graph, editing_station, set_is_over_station, set_is_over_edited_station, set_is_over_track
                );
            }
        }
    };

    let handle_mouse_up = move |_ev: MouseEvent| {
        canvas_viewport::handle_pan_end(&viewport_copy);

        if let Some(station_idx) = dragging_station.get() {
            if auto_layout_enabled.get() {
                if let Some(canvas_elem) = canvas_ref.get() {
                    let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
                    let rect = canvas.get_bounding_client_rect();
                    let x = f64::from(_ev.client_x()) - rect.left();
                    let y = f64::from(_ev.client_y()) - rect.top();

                    let zoom = zoom_level.get();
                    let pan_x = pan_offset_x.get();
                    let pan_y = pan_offset_y.get();
                    let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);

                    let mut current_graph = graph.get();
                    auto_layout::snap_to_angle(&mut current_graph, station_idx, world_x, world_y);
                    set_graph.set(current_graph);
                }
            }
            set_dragging_station.set(None);
        }
    };

    let handle_double_click = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let screen_x = f64::from(ev.client_x()) - rect.left();
            let screen_y = f64::from(ev.client_y()) - rect.top();

            let zoom = zoom_level.get();
            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();
            let (world_x, world_y) = screen_to_world(screen_x, screen_y, zoom, pan_x, pan_y);

            let current_graph = graph.get();

            if let Some(clicked_station) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) {
                set_editing_station.set(Some(clicked_station));
            } else if let Some(clicked_track) = hit_detection::find_track_at_position(&current_graph, world_x, world_y) {
                set_editing_track.set(Some(clicked_track));
            }
        }
    };

    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = f64::from(ev.client_x()) - rect.left();
            let mouse_y = f64::from(ev.client_y()) - rect.top();

            canvas_viewport::handle_zoom(&ev, mouse_x, mouse_y, &viewport_copy);
        }
    };

    (handle_mouse_down, handle_mouse_move, handle_mouse_up, handle_double_click, handle_wheel)
}

#[component]
#[must_use]
pub fn InfrastructureView(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (auto_layout_enabled, set_auto_layout_enabled) = create_signal(true);
    let (edit_mode, set_edit_mode) = create_signal(EditMode::None);
    let (selected_station, set_selected_station) = create_signal(None::<NodeIndex>);
    let (show_add_station, set_show_add_station) = create_signal(false);
    let (editing_station, set_editing_station) = create_signal(None::<NodeIndex>);
    let (editing_track, set_editing_track) = create_signal(None::<EdgeIndex>);
    let (show_delete_confirmation, set_show_delete_confirmation) = create_signal(false);
    let (station_to_delete, set_station_to_delete) = create_signal(None::<NodeIndex>);
    let (delete_affected_lines, set_delete_affected_lines) = create_signal(Vec::<String>::new());
    let (delete_station_name, set_delete_station_name) = create_signal(String::new());
    let (is_over_station, set_is_over_station) = create_signal(false);
    let (is_over_edited_station, set_is_over_edited_station) = create_signal(false);
    let (is_over_track, set_is_over_track) = create_signal(false);
    let (dragging_station, set_dragging_station) = create_signal(None::<NodeIndex>);

    let viewport = canvas_viewport::create_viewport_signals(false);
    let zoom_level = viewport.zoom_level;
    let pan_offset_x = viewport.pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;

    setup_auto_layout_effect(auto_layout_enabled, graph, set_graph, canvas_ref);

    let toggle_auto_layout = move |_| {
        let new_state = !auto_layout_enabled.get();
        set_auto_layout_enabled.set(new_state);

        if new_state {
            let mut current_graph = graph.get();
            for idx in current_graph.graph.node_indices() {
                current_graph.set_station_position(idx, (0.0, 0.0));
            }

            if let Some(canvas) = canvas_ref.get() {
                let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
                let height = f64::from(canvas_elem.client_height());
                auto_layout::apply_layout(&mut current_graph, height);
                set_graph.set(current_graph);
            }
        }
    };

    let handle_add_station = Rc::new(move |name: String, passing_loop: bool, connect_to: Option<NodeIndex>, platforms: Vec<crate::models::Platform>| {
        add_station_handler(name, passing_loop, connect_to, platforms, graph, set_graph, set_show_add_station);
    });

    let handle_edit_station = Rc::new(move |station_idx: NodeIndex, new_name: String, passing_loop: bool, platforms: Vec<crate::models::Platform>| {
        edit_station_handler(station_idx, new_name, passing_loop, platforms, graph, set_graph, set_editing_station);
    });

    let handle_delete_station = Rc::new(move |station_idx: NodeIndex| {
        delete_station_handler(station_idx, graph, lines, set_delete_affected_lines, set_station_to_delete, set_delete_station_name, set_show_delete_confirmation, set_editing_station);
    });

    let confirm_delete_station = Rc::new(move || {
        confirm_delete_station_handler(station_to_delete, graph, set_graph, lines, set_lines, set_show_delete_confirmation, set_station_to_delete);
    });

    let handle_edit_track = Rc::new(move |edge_idx: EdgeIndex, new_tracks: Vec<Track>, new_distance: Option<f64>| {
        edit_track_handler(edge_idx, new_tracks, new_distance, graph, set_graph, lines, set_lines, set_editing_track);
    });

    let handle_delete_track = Rc::new(move |edge_idx: EdgeIndex| {
        delete_track_handler(edge_idx, graph, set_graph, lines, set_lines, set_editing_track);
    });

    setup_render_effect(graph, zoom_level, pan_offset_x, pan_offset_y, canvas_ref);

    let (handle_mouse_down, handle_mouse_move, handle_mouse_up, handle_double_click, handle_wheel) = create_event_handlers(
        canvas_ref, edit_mode, selected_station, set_selected_station, graph, set_graph,
        editing_station, set_editing_station, set_editing_track,
        dragging_station, set_dragging_station, set_is_over_station, set_is_over_edited_station, set_is_over_track,
        auto_layout_enabled, &viewport
    );

    let handle_mouse_leave = move |_ev: MouseEvent| {
        canvas_viewport::handle_pan_end(&viewport);
        set_dragging_station.set(None);
        set_is_over_station.set(false);
        set_is_over_edited_station.set(false);
        set_is_over_track.set(false);
    };

    view! {
        <div class="infrastructure-view">
            <div class="infrastructure-toolbar">
                <button
                    class=move || if auto_layout_enabled.get() { "toolbar-button active" } else { "toolbar-button" }
                    on:click=toggle_auto_layout
                >
                    <i class="fa-solid fa-diagram-project"></i>
                    {move || if auto_layout_enabled.get() { " Auto Layout: On" } else { " Auto Layout: Off" }}
                </button>
                <button
                    class="toolbar-button"
                    on:click=move |_| set_show_add_station.set(true)
                >
                    <i class="fa-solid fa-circle-plus"></i>
                    " Add Station"
                </button>
                <button
                    class=move || if edit_mode.get() == EditMode::AddingTrack { "toolbar-button active" } else { "toolbar-button" }
                    on:click=move |_| {
                        if edit_mode.get() == EditMode::AddingTrack {
                            set_edit_mode.set(EditMode::None);
                            set_selected_station.set(None);
                        } else {
                            set_edit_mode.set(EditMode::AddingTrack);
                            set_selected_station.set(None);
                        }
                    }
                >
                    <i class="fa-solid fa-link"></i>
                    " Add Track"
                </button>
            </div>
            <div class="infrastructure-canvas-container">
                <canvas
                    node_ref=canvas_ref
                    class="infrastructure-canvas"
                    on:mousedown=handle_mouse_down
                    on:mousemove=handle_mouse_move
                    on:mouseup=handle_mouse_up
                    on:mouseleave=handle_mouse_leave
                    on:dblclick=handle_double_click
                    on:wheel=handle_wheel
                    on:contextmenu=|ev| ev.prevent_default()
                    style=move || get_canvas_cursor_style(dragging_station, edit_mode, is_over_edited_station, is_over_station, is_over_track)
                />
            </div>

            <AddStation
                is_open=show_add_station
                on_close=Rc::new(move || set_show_add_station.set(false))
                on_add=handle_add_station
                graph=graph
            />

            <EditStation
                editing_station=editing_station
                on_close=Rc::new(move || set_editing_station.set(None))
                on_save=handle_edit_station
                on_delete=handle_delete_station
                graph=graph
            />

            <EditTrack
                editing_track=editing_track
                on_close=Rc::new(move || set_editing_track.set(None))
                on_save=handle_edit_track
                on_delete=handle_delete_track
                graph=graph
                lines=lines
            />

            <DeleteStationConfirmation
                is_open=show_delete_confirmation
                station_name=delete_station_name
                affected_lines=delete_affected_lines
                on_cancel=Rc::new(move || set_show_delete_confirmation.set(false))
                on_confirm=confirm_delete_station
            />
        </div>
    }
}
