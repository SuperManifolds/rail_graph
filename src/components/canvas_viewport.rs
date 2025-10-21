use leptos::{batch, create_signal, ReadSignal, WriteSignal, SignalGet, SignalSet};
use web_sys::WheelEvent;

#[derive(Clone, Copy)]
pub struct ViewportSignals {
    pub zoom_level: ReadSignal<f64>,
    pub set_zoom_level: WriteSignal<f64>,
    pub zoom_level_x: Option<(ReadSignal<f64>, WriteSignal<f64>)>,
    pub pan_offset_x: ReadSignal<f64>,
    pub set_pan_offset_x: WriteSignal<f64>,
    pub pan_offset_y: ReadSignal<f64>,
    pub set_pan_offset_y: WriteSignal<f64>,
    pub is_panning: ReadSignal<bool>,
    pub set_is_panning: WriteSignal<bool>,
    pub last_mouse_pos: ReadSignal<(f64, f64)>,
    pub set_last_mouse_pos: WriteSignal<(f64, f64)>,
}

#[must_use]
pub fn create_viewport_signals(enable_horizontal_zoom: bool) -> ViewportSignals {
    let (zoom_level, set_zoom_level) = create_signal(1.0);
    let (pan_offset_x, set_pan_offset_x) = create_signal(0.0);
    let (pan_offset_y, set_pan_offset_y) = create_signal(0.0);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));

    let zoom_level_x = if enable_horizontal_zoom {
        let (zoom_x, set_zoom_x) = create_signal(1.0);
        Some((zoom_x, set_zoom_x))
    } else {
        None
    };

    ViewportSignals {
        zoom_level,
        set_zoom_level,
        zoom_level_x,
        pan_offset_x,
        set_pan_offset_x,
        pan_offset_y,
        set_pan_offset_y,
        is_panning,
        set_is_panning,
        last_mouse_pos,
        set_last_mouse_pos,
    }
}

pub fn handle_pan_start(
    x: f64,
    y: f64,
    viewport: &ViewportSignals,
) {
    viewport.set_is_panning.set(true);
    viewport.set_last_mouse_pos.set((x, y));
}

pub fn handle_pan_move(
    x: f64,
    y: f64,
    viewport: &ViewportSignals,
) {
    if !viewport.is_panning.get() {
        return;
    }

    let (last_x, last_y) = viewport.last_mouse_pos.get();
    let dx = x - last_x;
    let dy = y - last_y;

    let current_pan_x = viewport.pan_offset_x.get();
    let current_pan_y = viewport.pan_offset_y.get();

    batch(move || {
        viewport.set_pan_offset_x.set(current_pan_x + dx);
        viewport.set_pan_offset_y.set(current_pan_y + dy);
        viewport.set_last_mouse_pos.set((x, y));
    });
}

pub fn handle_pan_end(viewport: &ViewportSignals) {
    viewport.set_is_panning.set(false);
}

pub fn handle_zoom(
    ev: &WheelEvent,
    mouse_x: f64,
    mouse_y: f64,
    viewport: &ViewportSignals,
    min_zoom: Option<f64>,
    canvas_dimensions: Option<(f64, f64)>,
) {
    let delta = ev.delta_y();
    let shift_pressed = ev.shift_key();
    let alt_pressed = ev.alt_key();

    // No modifier = normal zoom
    // Shift = horizontal pan
    // Alt = horizontal zoom
    if shift_pressed && !alt_pressed {
        // Horizontal pan
        let pan_amount = -delta * 0.5;
        let current_pan_x = viewport.pan_offset_x.get();
        viewport.set_pan_offset_x.set(current_pan_x + pan_amount);
    } else if alt_pressed && !shift_pressed && viewport.zoom_level_x.is_some() {
        // Horizontal zoom
        let zoom_factor = if delta < 0.0 { 1.1 } else { 0.9 };
        apply_horizontal_zoom(zoom_factor, mouse_x, viewport);
    } else if !shift_pressed && !alt_pressed {
        // Normal zoom
        let zoom_factor = if delta < 0.0 { 1.1 } else { 0.9 };
        apply_normal_zoom(zoom_factor, mouse_x, mouse_y, viewport, min_zoom, canvas_dimensions);
    }
}

fn apply_horizontal_zoom(
    zoom_factor: f64,
    mouse_x: f64,
    viewport: &ViewportSignals,
) {
    let Some((zoom_x_signal, set_zoom_x)) = viewport.zoom_level_x else {
        return;
    };

    let old_zoom_x = zoom_x_signal.get();
    let new_zoom_x = (old_zoom_x * zoom_factor).clamp(0.1, 25.0);

    let pan_x = viewport.pan_offset_x.get();
    let new_pan_x = mouse_x - (mouse_x - pan_x) * (new_zoom_x / old_zoom_x);

    batch(move || {
        set_zoom_x.set(new_zoom_x);
        viewport.set_pan_offset_x.set(new_pan_x);
    });
}

fn apply_normal_zoom(
    zoom_factor: f64,
    mouse_x: f64,
    mouse_y: f64,
    viewport: &ViewportSignals,
    min_zoom: Option<f64>,
    canvas_dimensions: Option<(f64, f64)>,
) {
    let old_zoom = viewport.zoom_level.get();
    let min = min_zoom.unwrap_or(0.1);
    let new_zoom = (old_zoom * zoom_factor).clamp(min, 25.0);

    let pan_x = viewport.pan_offset_x.get();
    let pan_y = viewport.pan_offset_y.get();

    // Check if we hit the minimum zoom cap
    let hit_min_cap = new_zoom == min && old_zoom * zoom_factor < min;

    let new_pan_x = mouse_x - (mouse_x - pan_x) * (new_zoom / old_zoom);
    let new_pan_y = if let (true, Some((_, graph_height))) = (hit_min_cap, canvas_dimensions) {
        // When hitting the zoom cap, center the content vertically
        // At zoom < 1.0, content is smaller than viewport
        // We want to position it so: pan_y = graph_height * (1 - zoom) / 2
        graph_height * (1.0 - new_zoom) / 2.0
    } else {
        // Normal zoom-around-cursor behavior
        mouse_y - (mouse_y - pan_y) * (new_zoom / old_zoom)
    };

    batch(move || {
        viewport.set_zoom_level.set(new_zoom);
        viewport.set_pan_offset_x.set(new_pan_x);
        viewport.set_pan_offset_y.set(new_pan_y);
    });
}
