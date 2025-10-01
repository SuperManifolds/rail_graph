use super::ManualDepartureEditor;
use crate::constants::BASE_DATE;
use crate::models::{Line, ManualDeparture, RailwayGraph};
use leptos::*;
use std::rc::Rc;

#[component]
pub fn ManualDeparturesList(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label>"Manual Departures"</label>
            <div class="manual-departures-list">
                {
                    let on_save = on_save.clone();
                    move || {
                        edited_line.get().map(|line| {
                            let line_id = line.id.clone();
                            let current_graph = graph.get();
                            let station_names: Vec<String> = current_graph.get_line_stations(&line_id)
                                .into_iter()
                                .map(|(_, name)| name)
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
                            let current_graph = graph.get();
                            let station_names: Vec<String> = current_graph.get_line_stations(&line_id)
                                .into_iter()
                                .map(|(_, name)| name)
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
