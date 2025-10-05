use crate::models::RailwayGraph;
use crate::components::infrastructure_canvas::{auto_layout, station_renderer, track_renderer};
use leptos::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, MouseEvent, WheelEvent};

#[component]
pub fn InfrastructureView(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (auto_layout_enabled, set_auto_layout_enabled) = create_signal(true);

    // Zoom and pan state
    let (zoom_level, set_zoom_level) = create_signal(1.0);
    let (pan_offset_x, set_pan_offset_x) = create_signal(0.0);
    let (pan_offset_y, set_pan_offset_y) = create_signal(0.0);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));

    // Apply auto layout when enabled and graph changes
    create_effect(move |_| {
        if !auto_layout_enabled.get() {
            return;
        }

        let mut current_graph = graph.get();
        if current_graph.graph.node_count() > 0 {
            let Some(canvas) = canvas_ref.get() else { return };
            let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
            let height = canvas_elem.client_height() as f64;
            auto_layout::apply_layout(&mut current_graph, height);
            set_graph.set(current_graph);
        }
    });

    let toggle_auto_layout = move |_| {
        set_auto_layout_enabled.update(|enabled| *enabled = !*enabled);
    };

    // Re-render when graph or viewport changes
    create_effect(move |_| {
        let current_graph = graph.get();
        let _ = zoom_level.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();

        let Some(canvas) = canvas_ref.get() else { return };

        // Update canvas size to match container
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

        draw_infrastructure(&ctx, &current_graph, (container_width as f64, container_height as f64), zoom, pan_x, pan_y);
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
                <button
                    class=move || if auto_layout_enabled.get() { "toolbar-button active" } else { "toolbar-button" }
                    on:click=toggle_auto_layout
                >
                    <i class="fa-solid fa-diagram-project"></i>
                    {move || if auto_layout_enabled.get() { " Auto Layout: On" } else { " Auto Layout: Off" }}
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
                    on:wheel=handle_wheel
                    on:contextmenu=|ev| ev.prevent_default()
                    style="cursor: grab;"
                />
            </div>
        </div>
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

    // Draw tracks first so they're behind nodes
    track_renderer::draw_tracks(ctx, graph, zoom);

    // Draw stations on top
    station_renderer::draw_stations(ctx, graph, zoom);

    // Restore context
    ctx.restore();
}
