use leptos::{component, IntoView, ReadSignal, SignalGet, view};
use crate::conflict::Conflict;
use crate::models::{RailwayGraph, Stations};

#[component]
#[must_use]
pub fn ConflictTooltip(
    hovered_conflict: ReadSignal<Option<(Conflict, f64, f64)>>,
    graph: ReadSignal<RailwayGraph>,
) -> impl IntoView {
    view! {
        {move || {
            if let Some((conflict, tooltip_x, tooltip_y)) = hovered_conflict.get() {
                let current_graph = graph.get();

                // Convert station indices to NodeIndex by looking up in full graph station list
                let all_stations = current_graph.get_all_stations_ordered();
                let station1_node_idx = all_stations.get(conflict.station1_idx).map(|(idx, _)| *idx);
                let station2_node_idx = all_stations.get(conflict.station2_idx).map(|(idx, _)| *idx);

                // Get node names from the graph
                let station1_name = station1_node_idx
                    .and_then(|idx| current_graph.get_station_name(idx))
                    .unwrap_or("Unknown")
                    .to_string();
                let station2_name = station2_node_idx
                    .and_then(|idx| current_graph.get_station_name(idx))
                    .unwrap_or("Unknown")
                    .to_string();

                let message = conflict.format_message(&station1_name, &station2_name, &current_graph);
                let timestamp = conflict.time.format("%H:%M:%S");
                let tooltip_text = format!("{timestamp} - {message}");

                view! {
                    <div
                        class="conflict-tooltip"
                        style=format!("left: {}px; top: {}px;", tooltip_x + 10.0, tooltip_y - 30.0)
                    >
                        {tooltip_text}
                    </div>
                }.into_view()
            } else {
                view! { <div class="conflict-tooltip-hidden"></div> }.into_view()
            }
        }}
    }
}