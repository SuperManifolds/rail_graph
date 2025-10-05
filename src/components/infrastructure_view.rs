use crate::models::RailwayGraph;
use leptos::*;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, MouseEvent, WheelEvent};

const NODE_RADIUS: f64 = 8.0;
const TRACK_OFFSET: f64 = 3.0; // Offset for double track lines

#[component]
pub fn InfrastructureView(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (canvas_size, _set_canvas_size) = create_signal((1200.0, 800.0));
    let (trigger_layout, set_trigger_layout) = create_signal(0);

    // Zoom and pan state
    let (zoom_level, set_zoom_level) = create_signal(1.0);
    let (pan_offset_x, set_pan_offset_x) = create_signal(0.0);
    let (pan_offset_y, set_pan_offset_y) = create_signal(0.0);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));

    // Initialize layout when graph changes (if stations don't have positions) or when triggered
    create_effect(move |_| {
        let _ = trigger_layout.get();
        let mut current_graph = graph.get();
        let needs_layout = current_graph
            .graph
            .node_indices()
            .any(|idx| current_graph.get_station_position(idx).is_none());

        if needs_layout && current_graph.graph.node_count() > 0 {
            apply_force_layout(&mut current_graph, canvas_size.get().1);
            set_graph.set(current_graph);
        }
    });

    let on_auto_layout = move |_| {
        let mut current_graph = graph.get();
        if current_graph.graph.node_count() > 0 {
            // Clear all positions to force relayout
            for idx in current_graph.graph.node_indices() {
                current_graph.set_station_position(idx, (0.0, 0.0));
            }
            apply_force_layout(&mut current_graph, canvas_size.get().1);
            set_graph.set(current_graph);
            set_trigger_layout.update(|n| *n += 1);
        }
    };

    // Re-render when graph or viewport changes
    create_effect(move |_| {
        let current_graph = graph.get();
        let _ = zoom_level.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();

        let Some(canvas) = canvas_ref.get() else { return };
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

        draw_infrastructure(&ctx, &current_graph, canvas_size.get(), zoom, pan_x, pan_y);
    });

    // Mouse event handlers
    let handle_mouse_down = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = ev.client_x() as f64 - rect.left();
            let y = ev.client_y() as f64 - rect.top();

            // Right click or ctrl+click to pan
            if ev.button() == 2 || ev.ctrl_key() || ev.button() == 0 {
                set_is_panning.set(true);
                set_last_mouse_pos.set((x, y));
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

                batch(move || {
                    set_pan_offset_x.set(current_pan_x + dx);
                    set_pan_offset_y.set(current_pan_y + dy);
                    set_last_mouse_pos.set((x, y));
                });
            }
        }
    };

    let handle_mouse_up = move |_ev: MouseEvent| {
        set_is_panning.set(false);
    };

    let handle_mouse_leave = move |_ev: MouseEvent| {
        set_is_panning.set(false);
    };

    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = ev.client_x() as f64 - rect.left();
            let mouse_y = ev.client_y() as f64 - rect.top();

            let delta = ev.delta_y();
            let zoom_factor = if delta < 0.0 { 1.1 } else { 0.9 };

            let old_zoom = zoom_level.get();
            let new_zoom = (old_zoom * zoom_factor).clamp(0.1, 25.0);

            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();

            let new_pan_x = mouse_x - (mouse_x - pan_x) * (new_zoom / old_zoom);
            let new_pan_y = mouse_y - (mouse_y - pan_y) * (new_zoom / old_zoom);

            batch(move || {
                set_zoom_level.set(new_zoom);
                set_pan_offset_x.set(new_pan_x);
                set_pan_offset_y.set(new_pan_y);
            });
        }
    };

    view! {
        <div class="infrastructure-view">
            <div class="infrastructure-toolbar">
                <button class="toolbar-button" on:click=on_auto_layout>
                    <i class="fa-solid fa-diagram-project"></i>
                    " Auto Layout"
                </button>
            </div>
            <div class="infrastructure-canvas-container">
                <canvas
                    node_ref=canvas_ref
                    width=move || canvas_size.get().0
                    height=move || canvas_size.get().1
                    class="infrastructure-canvas"
                    on:mousedown=handle_mouse_down
                    on:mousemove=handle_mouse_move
                    on:mouseup=handle_mouse_up
                    on:mouseleave=handle_mouse_leave
                    on:wheel=handle_wheel
                    on:contextmenu=|ev| ev.prevent_default()
                    style="cursor: grab;"
                />
            </div>
        </div>
    }
}

fn apply_force_layout(graph: &mut RailwayGraph, height: f64) {
    let node_count = graph.graph.node_count();
    if node_count == 0 {
        return;
    }

    let station_spacing = 120.0;
    let start_x = 150.0;
    let start_y = height / 2.0;

    // Find a starting node (node with fewest connections)
    let start_node = graph
        .graph
        .node_indices()
        .min_by_key(|&idx| {
            let outgoing = graph.graph.edges(idx).count();
            let incoming = graph.graph.edges_directed(idx, Direction::Incoming).count();
            outgoing + incoming
        })
        .unwrap();

    let mut visited = std::collections::HashSet::new();
    let mut available_directions = vec![
        0.0,                                    // Right
        std::f64::consts::PI / 4.0,            // Down-right
        -std::f64::consts::PI / 4.0,           // Up-right
        std::f64::consts::PI / 2.0,            // Down
        -std::f64::consts::PI / 2.0,           // Up
        3.0 * std::f64::consts::PI / 4.0,      // Down-left
        -3.0 * std::f64::consts::PI / 4.0,     // Up-left
    ];

    // Layout the main line and branches
    layout_line(
        graph,
        start_node,
        (start_x, start_y),
        0.0, // Start with horizontal direction (0 radians)
        station_spacing,
        &mut visited,
        &mut available_directions,
    );
}

fn layout_line(
    graph: &mut RailwayGraph,
    current_node: NodeIndex,
    position: (f64, f64),
    direction: f64,
    spacing: f64,
    visited: &mut std::collections::HashSet<NodeIndex>,
    available_directions: &mut Vec<f64>,
) {
    if visited.contains(&current_node) {
        return;
    }

    // Set position for current node
    graph.set_station_position(current_node, position);
    visited.insert(current_node);

    // Get all unvisited neighbors (both incoming and outgoing edges)
    let mut neighbors = Vec::new();

    // Outgoing edges
    for edge in graph.graph.edges(current_node) {
        let target = edge.target();
        if !visited.contains(&target) {
            neighbors.push(target);
        }
    }

    // Incoming edges (treat graph as undirected for layout purposes)
    for edge in graph.graph.edges_directed(current_node, Direction::Incoming) {
        let source = edge.source();
        if !visited.contains(&source) {
            neighbors.push(source);
        }
    }

    if neighbors.is_empty() {
        return;
    }

    // First neighbor continues in the same direction (main line)
    let main_neighbor = neighbors[0];
    let next_pos = (
        position.0 + direction.cos() * spacing,
        position.1 + direction.sin() * spacing,
    );
    layout_line(
        graph,
        main_neighbor,
        next_pos,
        direction,
        spacing,
        visited,
        available_directions,
    );

    // Additional neighbors are branches - pick from available directions
    for &branch_neighbor in neighbors.iter().skip(1) {
        if let Some(branch_dir) = available_directions.pop() {
            let branch_pos = (
                position.0 + branch_dir.cos() * spacing,
                position.1 + branch_dir.sin() * spacing,
            );

            layout_line(
                graph,
                branch_neighbor,
                branch_pos,
                branch_dir,
                spacing,
                visited,
                available_directions,
            );
        }
    }
}

fn draw_infrastructure(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    (width, height): (f64, f64),
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
) {
    // Clear canvas
    ctx.set_fill_style_str("#0a0a0a");
    ctx.fill_rect(0.0, 0.0, width, height);

    if graph.graph.node_count() == 0 {
        // Show message if no stations
        ctx.set_fill_style_str("#666");
        ctx.set_font("16px sans-serif");
        let _ = ctx.fill_text("No stations in network", width / 2.0 - 80.0, height / 2.0);
        return;
    }

    // Save context and apply transformations
    ctx.save();
    let _ = ctx.translate(pan_x, pan_y);
    let _ = ctx.scale(zoom, zoom);

    // Draw tracks (edges) first so they're behind nodes
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        let is_double = edge.weight().double_tracked;

        if is_double {
            // Draw two parallel lines for double track
            let dx = pos2.0 - pos1.0;
            let dy = pos2.1 - pos1.1;
            let len = (dx * dx + dy * dy).sqrt();
            let nx = -dy / len * TRACK_OFFSET;
            let ny = dx / len * TRACK_OFFSET;

            ctx.set_stroke_style_str("#555");
            ctx.set_line_width(2.0 / zoom);

            // First track
            ctx.begin_path();
            ctx.move_to(pos1.0 + nx, pos1.1 + ny);
            ctx.line_to(pos2.0 + nx, pos2.1 + ny);
            ctx.stroke();

            // Second track
            ctx.begin_path();
            ctx.move_to(pos1.0 - nx, pos1.1 - ny);
            ctx.line_to(pos2.0 - nx, pos2.1 - ny);
            ctx.stroke();
        } else {
            // Single track
            ctx.set_stroke_style_str("#444");
            ctx.set_line_width(2.0 / zoom);
            ctx.begin_path();
            ctx.move_to(pos1.0, pos1.1);
            ctx.line_to(pos2.0, pos2.1);
            ctx.stroke();
        }
    }

    // Draw stations as nodes
    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(name) = graph.get_station_name(idx) else { continue };

        // Draw node circle
        ctx.set_fill_style_str("#2a2a2a");
        ctx.set_stroke_style_str("#4a9eff");
        ctx.set_line_width(2.0 / zoom);
        ctx.begin_path();
        let _ = ctx.arc(pos.0, pos.1, NODE_RADIUS, 0.0, std::f64::consts::PI * 2.0);
        ctx.fill();
        ctx.stroke();

        // Draw station name (scale font size inversely with zoom)
        ctx.set_fill_style_str("#fff");
        let font_size = 14.0 / zoom;
        ctx.set_font(&format!("{}px sans-serif", font_size));
        let _ = ctx.fill_text(name, pos.0 + NODE_RADIUS + 5.0, pos.1 + 5.0);
    }

    // Restore context
    ctx.restore();
}
