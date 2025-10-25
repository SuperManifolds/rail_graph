use leptos::{component, IntoView, ReadSignal, SignalGet, view};
use crate::conflict::Conflict;
use crate::models::RailwayGraph;

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

                // Get all nodes from the full graph
                let all_nodes: Vec<_> = current_graph.graph.node_indices()
                    .filter_map(|idx| current_graph.graph.node_weight(idx).map(|node| (idx, node)))
                    .collect();

                // Get station names from the full graph using conflict indices
                let station1_name = all_nodes.get(conflict.station1_idx)
                    .map_or_else(|| "Unknown".to_string(), |(_, n)| n.display_name().to_string());
                let station2_name = all_nodes.get(conflict.station2_idx)
                    .map_or_else(|| "Unknown".to_string(), |(_, n)| n.display_name().to_string());

                let message = if conflict.conflict_type == crate::conflict::ConflictType::PlatformViolation {
                    let platform_name = conflict.platform_idx.and_then(|idx| {
                        all_nodes.get(conflict.station1_idx)
                            .and_then(|(_, n)| n.as_station())
                            .and_then(|s| s.platforms.get(idx))
                            .map(|p| p.name.as_str())
                    }).unwrap_or("?");
                    conflict.format_platform_message(&station1_name, platform_name)
                } else {
                    conflict.format_message(&station1_name, &station2_name)
                };
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