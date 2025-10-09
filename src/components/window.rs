use leptos::{wasm_bindgen, component, view, MaybeSignal, Signal, Children, Props, IntoView, store_value, create_signal, create_node_ref, html, provide_context, SignalSet, SignalGet, create_effect, web_sys, SignalGetUntracked, Show};
use wasm_bindgen::{prelude::*, JsCast};

// Global window z-index counter
static NEXT_Z_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(2000);

#[component]
pub fn Window(
    #[prop(into)] is_open: MaybeSignal<bool>,
    title: Signal<String>,
    on_close: impl Fn() + 'static,
    children: Children,
    #[prop(default = (1600.0, 1200.0))] max_size: (f64, f64),
) -> impl IntoView {
    // Random offset so windows don't stack exactly on top of each other
    // Use store_value to ensure this is only calculated once
    let initial_position = store_value({
        let offset_x = js_sys::Math::random() * 200.0;
        let offset_y = js_sys::Math::random() * 150.0;
        (100.0 + offset_x, 100.0 + offset_y)
    });

    let (position, set_position) = create_signal(initial_position.get_value());
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (drag_offset, set_drag_offset) = create_signal((0.0, 0.0));
    let (size, set_size) = create_signal((400.0, 300.0)); // Initial size, will be auto-adjusted
    let (is_resizing, set_is_resizing) = create_signal(false);
    let (resize_start, set_resize_start) = create_signal((0.0, 0.0));
    let (z_index, set_z_index) = create_signal(NEXT_Z_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
    let content_ref = create_node_ref::<html::Div>();
    let (resize_trigger, set_resize_trigger) = create_signal(0u32);

    let on_close = store_value(on_close);

    // Provide context before children are created
    provide_context(set_resize_trigger);
    let children = store_value(children());

    let bring_to_front = move || {
        set_z_index.set(NEXT_Z_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
    };

    // Auto-size function that can be called on demand
    let auto_size = move || {
        if let Some(content_el) = content_ref.get() {
            leptos::set_timeout(
                move || {
                    let content_width = f64::from(content_el.scroll_width());
                    let content_height = f64::from(content_el.scroll_height());

                    let header_height = 45.0;
                    let padding = 20.0;

                    let target_width = (content_width + padding).min(max_size.0).max(250.0);
                    let target_height = (content_height + header_height + padding).min(max_size.1).max(200.0);

                    set_size.set((target_width, target_height));
                },
                std::time::Duration::from_millis(10),
            );
        }
    };

    // Bring window to front when it opens and auto-size to content
    create_effect(move |prev_open| {
        let currently_open = is_open.get();
        if currently_open && prev_open != Some(true) {
            bring_to_front();
            auto_size();
        }
        currently_open
    });

    // Watch for resize trigger changes and auto-size
    create_effect(move |_| {
        let _ = resize_trigger.get(); // Track changes
        if is_open.get() {
            auto_size();
        }
    });

    let handle_mouse_down = move |ev: web_sys::MouseEvent| {
        bring_to_front();
        set_is_dragging.set(true);
        let (pos_x, pos_y) = position.get_untracked();
        set_drag_offset.set((f64::from(ev.client_x()) - pos_x, f64::from(ev.client_y()) - pos_y));
    };

    let handle_mouse_move = move |ev: web_sys::MouseEvent| {
        if is_dragging.try_get_untracked().unwrap_or(false) {
            if let Some((offset_x, offset_y)) = drag_offset.try_get_untracked() {
                let _ = set_position.try_set((f64::from(ev.client_x()) - offset_x, f64::from(ev.client_y()) - offset_y));
            }
        }
    };

    let handle_mouse_up = move |_: web_sys::MouseEvent| {
        let _ = set_is_dragging.try_set(false);
        let _ = set_is_resizing.try_set(false);
    };

    let handle_resize_down = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        bring_to_front();
        let _ = set_is_resizing.try_set(true);
        if let Some((width, height)) = size.try_get_untracked() {
            let _ = set_resize_start.try_set((f64::from(ev.client_x()) - width, f64::from(ev.client_y()) - height));
        }
    };

    let handle_resize_move = move |ev: web_sys::MouseEvent| {
        if is_resizing.try_get_untracked().unwrap_or(false) {
            if let Some((start_x, start_y)) = resize_start.try_get_untracked() {
                let new_width = (f64::from(ev.client_x()) - start_x).max(250.0);
                let new_height = (f64::from(ev.client_y()) - start_y).max(200.0);
                let _ = set_size.try_set((new_width, new_height));
            }
        }
    };

    create_effect(move |_| {
        if is_open.get() {
            let Some(window) = web_sys::window() else {
                leptos::logging::error!("Failed to get window");
                return;
            };
            let Some(document) = window.document() else {
                leptos::logging::error!("Failed to get document");
                return;
            };
            let Some(body) = document.body() else {
                leptos::logging::error!("Failed to get body");
                return;
            };

            let move_handler = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
                handle_mouse_move(ev.clone());
                handle_resize_move(ev);
            }) as Box<dyn FnMut(_)>);

            let up_handler = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
                handle_mouse_up(ev);
            }) as Box<dyn FnMut(_)>);

            let _ = body.add_event_listener_with_callback("mousemove", move_handler.as_ref().unchecked_ref());
            let _ = body.add_event_listener_with_callback("mouseup", up_handler.as_ref().unchecked_ref());

            move_handler.forget();
            up_handler.forget();
        }
    });

    view! {
        <Show when=move || is_open.get()>
            <div
                class="window-dialog"
                style=move || {
                    let (x, y) = position.get();
                    let (width, height) = size.get();
                    let z = z_index.get();
                    format!("left: {x}px; top: {y}px; width: {width}px; height: {height}px; z-index: {z};")
                }
                on:mousedown=move |_| bring_to_front()
            >
                <div class="window-header" on:mousedown=handle_mouse_down>
                    <h3>{move || title.get()}</h3>
                    <button class="close-button" on:click=move |_| on_close.with_value(|f| f())>"×"</button>
                </div>

                <div class="window-content" node_ref=content_ref>
                    {children.with_value(|c| c.clone())}
                </div>

                <div class="resize-handle" on:mousedown=handle_resize_down></div>
            </div>
        </Show>
    }
}
