use leptos::{batch, create_signal, ReadSignal, WriteSignal, SignalGet, SignalSet, SignalGetUntracked, create_effect, store_value, on_cleanup};
use std::time::Duration;
use wasm_bindgen::JsCast;
use web_sys::WheelEvent;

// WASD panning speed (pixels per frame at 60fps)
const WASD_PAN_SPEED: f64 = 10.0;

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

#[must_use]
pub fn create_viewport_signals_with_initial(enable_horizontal_zoom: bool, initial: crate::models::ViewportState) -> ViewportSignals {
    let (zoom_level, set_zoom_level) = create_signal(initial.zoom_level);
    let (pan_offset_x, set_pan_offset_x) = create_signal(initial.pan_offset_x);
    let (pan_offset_y, set_pan_offset_y) = create_signal(initial.pan_offset_y);
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));

    let zoom_level_x = if enable_horizontal_zoom {
        let initial_zoom_x = initial.zoom_level_x.unwrap_or(1.0);
        let (zoom_x, set_zoom_x) = create_signal(initial_zoom_x);
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
    let alt_pressed = ev.alt_key();

    // Scroll wheel zoom controls:
    // - No modifier = vertical zoom (Y-axis only)
    // - Alt = horizontal zoom (X-axis only, time-based views)
    // Note: Shift+scroll horizontal panning was removed due to momentum scrolling conflicts.
    // Use Space+mouse or WASD keys for panning instead.
    if alt_pressed && viewport.zoom_level_x.is_some() {
        // Horizontal zoom
        let zoom_factor = if delta < 0.0 { 1.1 } else { 0.9 };
        apply_horizontal_zoom(zoom_factor, mouse_x, viewport);
    } else if !alt_pressed {
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

fn calculate_wasd_pan_delta(
    w_pressed: ReadSignal<bool>,
    a_pressed: ReadSignal<bool>,
    s_pressed: ReadSignal<bool>,
    d_pressed: ReadSignal<bool>,
) -> (f64, f64) {
    let mut dx = 0.0;
    let mut dy = 0.0;

    if w_pressed.get_untracked() {
        dy += WASD_PAN_SPEED;
    }
    if s_pressed.get_untracked() {
        dy -= WASD_PAN_SPEED;
    }
    if a_pressed.get_untracked() {
        dx += WASD_PAN_SPEED;
    }
    if d_pressed.get_untracked() {
        dx -= WASD_PAN_SPEED;
    }

    (dx, dy)
}

pub fn setup_wasd_panning(
    w_pressed: ReadSignal<bool>,
    a_pressed: ReadSignal<bool>,
    s_pressed: ReadSignal<bool>,
    d_pressed: ReadSignal<bool>,
    set_pan_offset_x: WriteSignal<f64>,
    set_pan_offset_y: WriteSignal<f64>,
    pan_offset_x: ReadSignal<f64>,
    pan_offset_y: ReadSignal<f64>,
) {
    use leptos::leptos_dom::helpers::IntervalHandle;
    let interval_handle = store_value(None::<IntervalHandle>);

    create_effect(move |_| {
        // Check key states and only create interval if at least one key is pressed.
        // This avoids unnecessary timer overhead when no keys are active.
        let any_key_pressed = w_pressed.get() || a_pressed.get() || s_pressed.get() || d_pressed.get();

        if any_key_pressed && interval_handle.get_value().is_none() {
            // Start interval for continuous panning
            let update_pan = move || {
                let (dx, dy) = calculate_wasd_pan_delta(w_pressed, a_pressed, s_pressed, d_pressed);

                // Safety check: skip update if no movement (e.g., key released mid-frame)
                if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
                    return;
                }

                batch(move || {
                    set_pan_offset_x.set(pan_offset_x.get_untracked() + dx);
                    set_pan_offset_y.set(pan_offset_y.get_untracked() + dy);
                });
            };

            let handle = leptos::leptos_dom::helpers::set_interval_with_handle(
                update_pan,
                Duration::from_millis(16) // ~60fps
            ).ok();
            interval_handle.set_value(handle);
        } else if !any_key_pressed {
            // Clear interval when no keys are pressed
            if let Some(handle) = interval_handle.get_value() {
                handle.clear();
                interval_handle.set_value(None);
            }
        }
    });

    on_cleanup(move || {
        if let Some(handle) = interval_handle.get_value() {
            handle.clear();
        }
    });
}

pub fn setup_keyboard_listeners(
    set_space_pressed: WriteSignal<bool>,
    set_w_pressed: WriteSignal<bool>,
    set_a_pressed: WriteSignal<bool>,
    set_s_pressed: WriteSignal<bool>,
    set_d_pressed: WriteSignal<bool>,
    viewport: &ViewportSignals,
) {
    let viewport_for_keyup = *viewport;

    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.repeat() {
            return;
        }

        // Don't handle keyboard shortcuts when typing in input fields
        if let Some(target) = ev.target() {
            if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                let tag_name = element.tag_name().to_lowercase();
                if tag_name == "input" || tag_name == "textarea" {
                    return;
                }
            }
        }

        match ev.code().as_str() {
            "Space" => {
                ev.prevent_default();
                set_space_pressed.set(true);
            }
            "KeyW" => set_w_pressed.set(true),
            "KeyA" => set_a_pressed.set(true),
            "KeyS" => set_s_pressed.set(true),
            "KeyD" => set_d_pressed.set(true),
            _ => {}
        }
    });

    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::keyup, move |ev| {
        // Don't handle keyboard shortcuts when typing in input fields
        if let Some(target) = ev.target() {
            if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                let tag_name = element.tag_name().to_lowercase();
                if tag_name == "input" || tag_name == "textarea" {
                    return;
                }
            }
        }

        match ev.code().as_str() {
            "Space" => {
                set_space_pressed.set(false);
                handle_pan_end(&viewport_for_keyup);
            }
            "KeyW" => set_w_pressed.set(false),
            "KeyA" => set_a_pressed.set(false),
            "KeyS" => set_s_pressed.set(false),
            "KeyD" => set_d_pressed.set(false),
            _ => {}
        }
    });
}
