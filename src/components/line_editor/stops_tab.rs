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
                            let current_graph = graph.get();

                            if line.route.is_empty() {
                                view! {
                                    <p class="no-stops">"No stops defined for this line yet. Import a CSV to set up the route."</p>
                                }.into_view()
                            } else {
                                // Build list of stations from route
                                let mut stations = Vec::new();

                                // Add first station
                                if let Some(segment) = line.route.first() {
                                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                    if let Some((from, _)) = current_graph.get_track_endpoints(edge_idx) {
                                        if let Some(name) = current_graph.get_station_name(from) {
                                            stations.push(name.to_string());
                                        }
                                    }
                                }

                                // Add stations from each segment
                                for segment in &line.route {
                                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                    if let Some((_, to)) = current_graph.get_track_endpoints(edge_idx) {
                                        if let Some(name) = current_graph.get_station_name(to) {
                                            stations.push(name.to_string());
                                        }
                                    }
                                }

                                view! {
                                    <div class="stops-header">
                                        <span>"Station"</span>
                                        <span>"Travel Time to Next"</span>
                                    </div>
                                    {stations.into_iter().enumerate().map(|(i, name)| {
                                        let travel_time_str = if i < line.route.len() {
                                            let minutes = line.route[i].duration.num_minutes();
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
