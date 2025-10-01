use crate::components::{
    frequency_input::FrequencyInput,
    manual_departures_list::ManualDeparturesList,
    tab_view::{Tab, TabPanel, TabView},
    time_input::TimeInput,
    window::Window,
};
use crate::models::{Line, ScheduleMode, Station};
use crate::constants::BASE_DATE;
use leptos::*;
use std::rc::Rc;

#[component]
fn GeneralTab(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
    active_tab: RwSignal<String>,
) -> impl IntoView {
    let on_save = store_value(on_save);
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "general")>
            <div class="line-editor-content">
                <div class="form-group">
                    <label>"Name"</label>
                    <input
                        type="text"
                        value=move || edited_line.get().map(|l| l.id.clone()).unwrap_or_default()
                        on:change={
                            let on_save = on_save.get_value();
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
                            let on_save = on_save.get_value();
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
            </div>
        </TabPanel>
    }
}

#[component]
fn ScheduleTab(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    stations: ReadSignal<Vec<Station>>,
    on_save: Rc<dyn Fn(Line)>,
    active_tab: RwSignal<String>,
) -> impl IntoView {
    let on_save = store_value(on_save);
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "schedule")>
            <div class="line-editor-content">
                <div class="form-group">
                    <label>
                        <input
                            type="checkbox"
                            checked=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Auto)
                            on:change={
                                let on_save = on_save.get_value();
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
                        let on_save = on_save.get_value();
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
                        on_save=on_save.get_value()
                    />
                </Show>
            </div>
        </TabPanel>
    }
}

#[component]
fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    stations: ReadSignal<Vec<Station>>,
    set_stations: WriteSignal<Vec<Station>>,
    active_tab: RwSignal<String>,
) -> impl IntoView {
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "stops")>
            <div class="line-editor-content">
                <div class="stops-list">
                    {move || {
                        edited_line.get().map(|line| {
                            let line_id = line.id.clone();

                            // Get all stations that have a time for this line
                            let line_stations: Vec<_> = stations.get()
                                .into_iter()
                                .filter(|s| s.times.contains_key(&line_id))
                                .collect();

                            view! {
                                <div class="stops-header">
                                    <span>"Station"</span>
                                    <span>"Travel Time from Start"</span>
                                </div>
                                {line_stations.into_iter().map(|station| {
                                    let station_name = station.name.clone();
                                    let station_name_for_display = station_name.clone();
                                    let station_name_for_value = station_name.clone();
                                    let station_name_for_change = station_name.clone();
                                    let line_id_for_value = line_id.clone();
                                    let line_id_for_change = line_id.clone();

                                    view! {
                                        <div class="stop-row">
                                            <span class="station-name">{station_name_for_display}</span>
                                            <TimeInput
                                                label=""
                                                value=Signal::derive(move || {
                                                    stations.get()
                                                        .iter()
                                                        .find(|s| s.name == station_name_for_value)
                                                        .and_then(|s| s.get_time(&line_id_for_value))
                                                        .unwrap_or_else(|| BASE_DATE.and_hms_opt(0, 0, 0).unwrap())
                                                })
                                                default_time="00:00"
                                                on_change={
                                                    Box::new(move |time| {
                                                        set_stations.update(|stations| {
                                                            if let Some(station) = stations.iter_mut().find(|s| s.name == station_name_for_change) {
                                                                station.times.insert(line_id_for_change.clone(), Some(time));
                                                            }
                                                        });
                                                    })
                                                }
                                            />
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            }
                        })
                    }}
                </div>
            </div>
        </TabPanel>
    }
}

#[component]
pub fn LineEditor(
    #[prop(into)] initial_line: MaybeSignal<Option<Line>>,
    is_open: Signal<bool>,
    set_is_open: impl Fn(bool) + 'static,
    stations: ReadSignal<Vec<Station>>,
    set_stations: WriteSignal<Vec<Station>>,
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
        edited_line
            .get()
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
                let on_save_stored = store_value(on_save);
                move || {
                    edited_line.get().map(|_line| {
                        let tabs = vec![
                            Tab { id: "general".to_string(), label: "General".to_string() },
                            Tab { id: "stops".to_string(), label: "Stops".to_string() },
                            Tab { id: "schedule".to_string(), label: "Schedule".to_string() },
                        ];
                        view! {
                            <TabView tabs=tabs active_tab=active_tab>
                                <GeneralTab
                                    edited_line=edited_line
                                    set_edited_line=set_edited_line
                                    on_save=on_save_stored.get_value()
                                    active_tab=active_tab
                                />
                                <StopsTab
                                    edited_line=edited_line
                                    stations=stations
                                    set_stations=set_stations
                                    active_tab=active_tab
                                />
                                <ScheduleTab
                                    edited_line=edited_line
                                    set_edited_line=set_edited_line
                                    stations=stations
                                    on_save=on_save_stored.get_value()
                                    active_tab=active_tab
                                />
                            </TabView>
                        }
                    })
                }
            }
        </Window>
    }
}
