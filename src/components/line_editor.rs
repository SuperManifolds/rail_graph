use crate::components::{frequency_input::FrequencyInput, time_input::TimeInput, manual_departure_editor::ManualDepartureEditor};
use crate::models::{Line, ScheduleMode, ManualDeparture, Station};
use crate::constants::BASE_DATE;
use leptos::*;
use std::rc::Rc;
use wasm_bindgen::{prelude::*, JsCast};

#[component]
pub fn LineEditor(
    #[prop(into)] initial_line: MaybeSignal<Option<Line>>,
    is_open: ReadSignal<bool>,
    set_is_open: WriteSignal<bool>,
    stations: ReadSignal<Vec<Station>>,
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
    let (size, set_size) = create_signal((500.0, 400.0));
    let (is_resizing, set_is_resizing) = create_signal(false);
    let (resize_start, set_resize_start) = create_signal((0.0, 0.0));

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
        set_is_resizing.set(false);
    };

    let handle_resize_down = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        set_is_resizing.set(true);
        let (width, height) = size.get_untracked();
        set_resize_start.set((ev.client_x() as f64 - width, ev.client_y() as f64 - height));
    };

    let handle_resize_move = move |ev: web_sys::MouseEvent| {
        if is_resizing.get_untracked() {
            let (start_x, start_y) = resize_start.get_untracked();
            let new_width = (ev.client_x() as f64 - start_x).max(250.0);
            let new_height = (ev.client_y() as f64 - start_y).max(200.0);
            set_size.set((new_width, new_height));
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
                                let (width, height) = size.get();
                                format!("left: {}px; top: {}px; width: {}px; height: {}px;", x, y, width, height)
                            }
                        >
                            <div class="line-editor-header" on:mousedown=handle_mouse_down>
                                <h3>"Edit Line: " {line_id.clone()}</h3>
                                <button class="close-button" on:click=close_dialog>"×"</button>
                            </div>

                    <div class="line-editor-content">
                        {
                            let on_save_name = on_save.clone();
                            let on_save_color = on_save.clone();
                            let on_save_mode = on_save.clone();
                            let on_save_auto = on_save.clone();
                            let on_save_manual = on_save.clone();
                            view! {
                                <div class="form-group">
                                    <label>"Name"</label>
                                    <input
                                        type="text"
                                        value=move || edited_line.get().map(|l| l.id.clone()).unwrap_or_default()
                                        on:change={
                                            let on_save = on_save_name.clone();
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
                                    let on_save = on_save_color.clone();
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
                            <label>
                                <input
                                    type="checkbox"
                                    checked=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Auto)
                                    on:change={
                                        let on_save = on_save_mode.clone();
                                        move |ev| {
                                            let is_auto = event_target_checked(&ev);
                                            let mode = if is_auto { ScheduleMode::Auto } else { ScheduleMode::Manual };
                                            if let Some(mut updated_line) = edited_line.get_untracked() {
                                                updated_line.schedule_mode = mode;
                                                set_edited_line.set(Some(updated_line.clone()));
                                                on_save(updated_line);
                                            }
                                        }
                                    }
                                />
                                " Auto Schedule"
                            </label>
                        </div>

                        <Show when=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Auto)>
                            {
                                let on_save = on_save_auto.clone();
                                move || {
                                    view! {
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
                                    }
                                }
                            }
                        </Show>

                        <Show when=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Manual)>
                            {
                                let on_save_manual_inner = on_save_manual.clone();
                                move || {
                                    let on_save = on_save_manual_inner.clone();
                                    view! {
                                        <div class="form-group">
                                            <label>"Manual Departures"</label>
                                            <div class="manual-departures-list">
                                                {
                                                    let on_save = on_save.clone();
                                                    move || {
                                                        edited_line.get().map(|line| {
                                                            let line_id = line.id.clone();
                                                            let station_names: Vec<String> = stations.get()
                                                                .iter()
                                                                .filter(|s| s.get_time(&line_id).is_some())
                                                                .map(|s| s.name.clone())
                                                                .collect();
                                                            line.manual_departures.iter().enumerate().map(|(idx, dep)| {
                                                                let on_save = on_save.clone();
                                                                let station_names = station_names.clone();
                                                                view! {
                                                                    <ManualDepartureEditor
                                                                        index=idx
                                                                        departure=dep.clone()
                                                                        station_names=station_names
                                                                        on_update={
                                                                            let on_save = on_save.clone();
                                                                            move |idx, updated_dep| {
                                                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                                    if let Some(departure) = updated_line.manual_departures.get_mut(idx) {
                                                                                        *departure = updated_dep;
                                                                                    }
                                                                                    set_edited_line.set(Some(updated_line.clone()));
                                                                                    on_save(updated_line);
                                                                                }
                                                                            }
                                                                        }
                                                                        on_remove={
                                                                            move |idx| {
                                                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                                    updated_line.manual_departures.remove(idx);
                                                                                    set_edited_line.set(Some(updated_line.clone()));
                                                                                    on_save(updated_line);
                                                                                }
                                                                            }
                                                                        }
                                                                    />
                                                                }
                                                        }).collect::<Vec<_>>()
                                                    }).unwrap_or_default()
                                                    }
                                                }
                                            </div>
                                            <button
                                                class="add-departure"
                                                on:click={
                                                    let on_save = on_save.clone();
                                                    move |_| {
                                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                                            let line_id = updated_line.id.clone();
                                                            let station_names: Vec<String> = stations.get()
                                                                .iter()
                                                                .filter(|s| s.get_time(&line_id).is_some())
                                                                .map(|s| s.name.clone())
                                                                .collect();

                                                            let from_station = station_names.first().cloned().unwrap_or_else(|| "Station A".to_string());
                                                            let to_station = station_names.last().cloned().unwrap_or_else(|| "Station B".to_string());

                                                            let new_departure = ManualDeparture {
                                                                time: BASE_DATE.and_hms_opt(8, 0, 0).unwrap(),
                                                                from_station,
                                                                to_station,
                                                            };
                                                            updated_line.manual_departures.push(new_departure);
                                                            set_edited_line.set(Some(updated_line.clone()));
                                                            on_save(updated_line);
                                                        }
                                                    }
                                                }
                                            >
                                                "+ Add Departure"
                                            </button>
                                        </div>
                                    }
                                }
                            }
                        </Show>
                            }
                        }
                    </div>

                    <div class="resize-handle" on:mousedown=handle_resize_down></div>

                        </div>
                    }
                })
            }}
        </Show>
    }
}
