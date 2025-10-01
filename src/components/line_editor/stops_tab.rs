use crate::components::tab_view::TabPanel;
use crate::models::{Line, RailwayGraph};
use leptos::*;

#[component]
pub fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    active_tab: RwSignal<String>,
) -> impl IntoView {
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "stops")>
            <div class="line-editor-content">
                <div class="stops-list">
                    {move || {
                        edited_line.get().map(|line| {
                            let line_id = line.id.clone();
                            let current_graph = graph.get();

                            // Get the stations on this line's path
                            let line_stations = current_graph.get_line_stations(&line_id);

                            if line_stations.is_empty() {
                                view! {
                                    <p class="no-stops">"No stops defined for this line yet. Import a CSV to set up the route."</p>
                                }.into_view()
                            } else {
                                view! {
                                    <div class="stops-header">
                                        <span>"Station"</span>
                                        <span>"Travel Time to Next"</span>
                                    </div>
                                    {line_stations.into_iter().enumerate().map(|(i, (_idx, name))| {
                                        let line_path = current_graph.get_line_path(&line_id);
                                        let travel_time_str = if i < line_path.len() {
                                            let travel_time = line_path[i].2;
                                            let minutes = travel_time.num_minutes();
                                            format!("{} min", minutes)
                                        } else {
                                            "-".to_string()
                                        };

                                        view! {
                                            <div class="stop-row">
                                                <span class="station-name">{name}</span>
                                                <span class="travel-time">{travel_time_str}</span>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                }.into_view()
                            }
                        })
                    }}
                </div>
            </div>
        </TabPanel>
    }
}
