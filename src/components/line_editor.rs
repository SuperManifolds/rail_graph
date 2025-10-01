use crate::components::{
    frequency_input::FrequencyInput,
    time_input::TimeInput,
    manual_departures_list::ManualDeparturesList,
    window::Window,
    tab_view::{TabView, TabPanel, Tab}
};
use crate::models::{Line, ScheduleMode, Station};
use leptos::*;
use std::rc::Rc;

#[component]
pub fn LineEditor(
    #[prop(into)] initial_line: MaybeSignal<Option<Line>>,
    is_open: Signal<bool>,
    set_is_open: impl Fn(bool) + 'static,
    stations: ReadSignal<Vec<Station>>,
    on_save: impl Fn(Line) + 'static,
) -> impl IntoView {
    let (edited_line, set_edited_line) = create_signal(None::<Line>);
    let active_tab = create_rw_signal("general".to_string());

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

    let on_save = Rc::new(on_save);
    let set_is_open = store_value(set_is_open);

    let close_dialog = move || {
        set_is_open.with_value(|f| f(false));
    };

    let window_title = Signal::derive(move || {
        edited_line.get()
            .map(|line| format!("Edit Line: {}", line.id))
            .unwrap_or_else(|| "Edit Line".to_string())
    });

    let is_window_open = Signal::derive(move || is_open.get() && edited_line.get().is_some());

    view! {
        <Window
            is_open=is_window_open
            title=window_title
            on_close=close_dialog
        >
            {
                let on_save_stored = store_value(on_save.clone());
                move || {
                    edited_line.get().map(|_line| {
                        let tabs = vec![
                            Tab { id: "general".to_string(), label: "General".to_string() },
                        ];
                        view! {
                    <TabView tabs=tabs active_tab=active_tab>
                        {
                            let on_save_name = on_save_stored.get_value();
                            let on_save_color = on_save_stored.get_value();
                            let on_save_mode = on_save_stored.get_value();
                            let on_save_auto = on_save_stored.get_value();
                            let on_save_manual = on_save_stored.get_value();
                            view! {
                                <TabPanel when=Signal::derive(move || active_tab.get() == "general")>
                                <div class="line-editor-content">
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
                            <ManualDeparturesList
                                edited_line=edited_line
                                set_edited_line=set_edited_line
                                stations=stations
                                on_save=on_save_manual.clone()
                            />
                        </Show>
                    </div>
                                </TabPanel>
                            }
                        }
                    </TabView>
                        }
                    })
                }
            }
        </Window>
    }
}
