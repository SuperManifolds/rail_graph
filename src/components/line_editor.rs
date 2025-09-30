use crate::components::{frequency_input::FrequencyInput, time_input::TimeInput};
use crate::models::Line;
use leptos::*;
use std::rc::Rc;
use wasm_bindgen::{prelude::*, JsCast};

#[component]
pub fn LineEditor(
    #[prop(into)] initial_line: MaybeSignal<Option<Line>>,
    is_open: ReadSignal<bool>,
    set_is_open: WriteSignal<bool>,
    on_save: impl Fn(Line) + 'static,
) -> impl IntoView {
    let (edited_line, set_edited_line) = create_signal(None::<Line>);

    // Reset edited_line when dialog opens (not when initial_line changes)
    create_effect(move |prev_open| {
        let currently_open = is_open.get();
        if currently_open && prev_open != Some(true) {
            if let Some(line) = initial_line.get_untracked() {
                set_edited_line.set(Some(line));
            }
        }
        currently_open
    });
    let (position, set_position) = create_signal((0.0, 0.0));
    let (is_dragging, set_is_dragging) = create_signal(false);
    let (drag_offset, set_drag_offset) = create_signal((0.0, 0.0));

    let on_save = Rc::new(on_save);

    let close_dialog = move |_| {
        set_is_open.set(false);
    };

    let handle_mouse_down = move |ev: web_sys::MouseEvent| {
        set_is_dragging.set(true);
        let (pos_x, pos_y) = position.get_untracked();
        set_drag_offset.set((ev.client_x() as f64 - pos_x, ev.client_y() as f64 - pos_y));
    };

    let handle_mouse_move = move |ev: web_sys::MouseEvent| {
        if is_dragging.get_untracked() {
            let (offset_x, offset_y) = drag_offset.get_untracked();
            set_position.set((ev.client_x() as f64 - offset_x, ev.client_y() as f64 - offset_y));
        }
    };

    let handle_mouse_up = move |_: web_sys::MouseEvent| {
        set_is_dragging.set(false);
    };

    create_effect(move |_| {
        if is_open.get() {
            let document = web_sys::window().unwrap().document().unwrap();
            let body = document.body().unwrap();

            let move_handler = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
                handle_mouse_move(ev);
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
        <Show when=move || is_open.get() && edited_line.get().is_some()>
            {
                let on_save = on_save.clone();
                move || {
                    edited_line.get().map(|line| {
                        let line_id = line.id.clone();
                        view! {
                        <div
                            class="line-editor-dialog"
                            style=move || {
                                let (x, y) = position.get();
                                format!("left: {}px; top: {}px;", x, y)
                            }
                        >
                            <div class="line-editor-header" on:mousedown=handle_mouse_down>
                                <h3>"Edit Line: " {line_id.clone()}</h3>
                                <button class="close-button" on:click=close_dialog>"×"</button>
                            </div>

                    <div class="line-editor-content">
                        <div class="form-group">
                            <label>"Name"</label>
                            <input
                                type="text"
                                value=move || edited_line.get().map(|l| l.id.clone()).unwrap_or_default()
                                on:change={
                                    let on_save = on_save.clone();
                                    move |ev| {
                                        let name = event_target_value(&ev);
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            updated_line.id = name;
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    }
                                }
                            />
                        </div>

                        <div class="form-group">
                            <label>"Color"</label>
                            <input
                                type="color"
                                value=move || edited_line.get().map(|l| l.color).unwrap_or_default()
                                on:change={
                                    let on_save = on_save.clone();
                                    move |ev| {
                                        let color = event_target_value(&ev);
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            updated_line.color = color;
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    }
                                }
                            />
                        </div>

                        <div class="form-group">
                            <label>"Frequency"</label>
                            <FrequencyInput
                                frequency=Signal::derive(move || edited_line.get().map(|l| l.frequency).unwrap_or_default())
                                on_change={
                                    let on_save = on_save.clone();
                                    move |freq| {
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            updated_line.frequency = freq;
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    }
                                }
                            />
                        </div>

                        <div class="form-group">
                            <label>"First Departure"</label>
                            <TimeInput
                                label=""
                                value=Signal::derive(move || edited_line.get().map(|l| l.first_departure).unwrap_or_default())
                                default_time="05:00"
                                on_change={
                                    let on_save = on_save.clone();
                                    Box::new(move |time| {
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            updated_line.first_departure = time;
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    })
                                }
                            />
                        </div>

                        <div class="form-group">
                            <label>"Return First Departure"</label>
                            <TimeInput
                                label=""
                                value=Signal::derive(move || edited_line.get().map(|l| l.return_first_departure).unwrap_or_default())
                                default_time="06:00"
                                on_change={
                                    let on_save = on_save.clone();
                                    Box::new(move |time| {
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            updated_line.return_first_departure = time;
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    })
                                }
                            />
                        </div>
                    </div>

                        </div>
                    }
                })
            }}
        </Show>
    }
}
