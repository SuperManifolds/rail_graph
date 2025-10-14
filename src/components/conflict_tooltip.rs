use leptos::{component, IntoView, ReadSignal, Signal, SignalGet, view};
use crate::conflict::Conflict;
use crate::models::Node;
use petgraph::stable_graph::NodeIndex;

#[component]
#[must_use]
pub fn ConflictTooltip(
    hovered_conflict: ReadSignal<Option<(Conflict, f64, f64)>>,
    stations: Signal<Vec<(NodeIndex, Node)>>,
) -> impl IntoView {
    view! {
        {move || {
            if let Some((conflict, tooltip_x, tooltip_y)) = hovered_conflict.get() {
                let current_stations = stations.get();

                // Get node names
                let station1_name = current_stations.get(conflict.station1_idx)
                    .map_or("Unknown".to_string(), |(_, n)| n.display_name());
                let station2_name = current_stations.get(conflict.station2_idx)
                    .map_or("Unknown".to_string(), |(_, n)| n.display_name());

                let message = conflict.format_message(&station1_name, &station2_name);
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