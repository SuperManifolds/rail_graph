use leptos::*;
use wasm_bindgen::{prelude::*, JsCast};

// Global window z-index counter
static NEXT_Z_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(2000);

#[component]
pub fn Window(
    #[prop(into)] is_open: MaybeSignal<bool>,
    title: Signal<String>,
    on_close: impl Fn() + 'static,
    children: Children,
) -> impl IntoView {
    let (position, set_position) = create_signal((100.0, 100.0));
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (drag_offset, set_drag_offset) = create_signal((0.0, 0.0));
    let (size, set_size) = create_signal((500.0, 400.0));
    let (is_resizing, set_is_resizing) = create_signal(false);
    let (resize_start, set_resize_start) = create_signal((0.0, 0.0));
    let (z_index, set_z_index) = create_signal(NEXT_Z_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst));

    let on_close = store_value(on_close);
    let children = store_value(children());

    let bring_to_front = move || {
        set_z_index.set(NEXT_Z_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
    };

    let handle_mouse_down = move |ev: web_sys::MouseEvent| {
        bring_to_front();
        set_is_dragging.set(true);
        let (pos_x, pos_y) = position.get_untracked();
        set_drag_offset.set((ev.client_x() as f64 - pos_x, ev.client_y() as f64 - pos_y));
    };

    let handle_mouse_move = move |ev: web_sys::MouseEvent| {
        if is_dragging.try_get_untracked().unwrap_or(false) {
            if let Some((offset_x, offset_y)) = drag_offset.try_get_untracked() {
                let _ = set_position.try_set((ev.client_x() as f64 - offset_x, ev.client_y() as f64 - offset_y));
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
            let _ = set_resize_start.try_set((ev.client_x() as f64 - width, ev.client_y() as f64 - height));
        }
    };

    let handle_resize_move = move |ev: web_sys::MouseEvent| {
        if is_resizing.try_get_untracked().unwrap_or(false) {
            if let Some((start_x, start_y)) = resize_start.try_get_untracked() {
                let new_width = (ev.client_x() as f64 - start_x).max(250.0);
                let new_height = (ev.client_y() as f64 - start_y).max(200.0);
                let _ = set_size.try_set((new_width, new_height));
            }
        }
    };

    create_effect(move |_| {
        if is_open.get() {
            let document = web_sys::window().unwrap().document().unwrap();
            let body = document.body().unwrap();

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
                    format!("left: {}px; top: {}px; width: {}px; height: {}px; z-index: {};", x, y, width, height, z)
                }
                on:mousedown=move |_| bring_to_front()
            >
                <div class="window-header" on:mousedown=handle_mouse_down>
                    <h3>{move || title.get()}</h3>
                    <button class="close-button" on:click=move |_| on_close.with_value(|f| f())>"×"</button>
                </div>

                <div class="window-content">
                    {children.with_value(|c| c.clone())}
                </div>

                <div class="resize-handle" on:mousedown=handle_resize_down></div>
            </div>
        </Show>
    }
}
