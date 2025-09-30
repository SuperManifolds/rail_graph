use crate::components::time_input::TimeInput;
use crate::models::ManualDeparture;
use leptos::*;

#[component]
pub fn ManualDepartureEditor(
    index: usize,
    departure: ManualDeparture,
    station_names: Vec<String>,
    on_update: impl Fn(usize, ManualDeparture) + 'static,
    on_remove: impl Fn(usize) + 'static,
) -> impl IntoView {
    let (local_departure, set_local_departure) = create_signal(departure.clone());

    let on_update = store_value(on_update);
    let on_remove = store_value(on_remove);

    view! {
        <div class="manual-departure-item">
            <TimeInput
                label=""
                value=Signal::derive(move || local_departure.get().time)
                default_time="08:00"
                on_change={
                    Box::new(move |time| {
                        set_local_departure.update(|dep| dep.time = time);
                        on_update.with_value(|f| f(index, local_departure.get_untracked()));
                    })
                }
            />
            <select
                class="station-input"
                on:change={
                    move |ev| {
                        let station = event_target_value(&ev);
                        set_local_departure.update(|dep| dep.from_station = station);
                        on_update.with_value(|f| f(index, local_departure.get_untracked()));
                    }
                }
            >
                {
                    station_names.iter().map(|name| {
                        let name_clone = name.clone();
                        view! {
                            <option value=name.clone() selected=move || local_departure.get().from_station == name_clone>
                                {name.clone()}
                            </option>
                        }
                    }).collect::<Vec<_>>()
                }
            </select>
            <span class="arrow">"→"</span>
            <select
                class="station-input"
                on:change={
                    move |ev| {
                        let station = event_target_value(&ev);
                        set_local_departure.update(|dep| dep.to_station = station);
                        on_update.with_value(|f| f(index, local_departure.get_untracked()));
                    }
                }
            >
                {
                    station_names.iter().map(|name| {
                        let name_clone = name.clone();
                        view! {
                            <option value=name.clone() selected=move || local_departure.get().to_station == name_clone>
                                {name.clone()}
                            </option>
                        }
                    }).collect::<Vec<_>>()
                }
            </select>
            <button
                class="remove-departure"
                on:click=move |_| on_remove.with_value(|f| f(index))
            >
                "×"
            </button>
        </div>
    }
}
