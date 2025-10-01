use crate::components::{tab_view::TabPanel, time_input::TimeInput};
use crate::constants::BASE_DATE;
use crate::models::{Line, Station};
use leptos::*;

#[component]
pub fn StopsTab(
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
