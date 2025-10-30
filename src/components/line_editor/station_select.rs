use crate::models::{Line, RailwayGraph, RouteDirection, StationPosition, ProjectSettings};
use leptos::{component, view, ReadSignal, IntoView, event_target_value, SignalGetUntracked, SignalGet};
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;

#[component]
pub fn StationSelect(
    available_stations: Vec<(String, NodeIndex)>,
    position: StationPosition,
    route_direction: RouteDirection,
    graph: ReadSignal<RailwayGraph>,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
    settings: ReadSignal<ProjectSettings>,
) -> impl IntoView {

    if available_stations.is_empty() {
        return view! {}.into_view();
    }

    let avail = available_stations.clone();
    let label = match position {
        StationPosition::Start => "+ Add stop at start...",
        StationPosition::End => "+ Add stop at end...",
    };

    view! {
        <div class="add-stop-row">
            <select
                class="station-select"
                on:change={
                    move |ev| {
                        let value = event_target_value(&ev);
                        if let Ok(idx) = value.parse::<usize>() {
                            if let Some(mut line) = edited_line.get_untracked() {
                                let node_idx = NodeIndex::new(idx);
                                let handedness = settings.get_untracked().track_handedness;

                                if line.add_station_to_route(
                                    node_idx,
                                    &graph.get(),
                                    route_direction,
                                    position,
                                    handedness,
                                ) {
                                    on_save(line);
                                }
                            }
                        }
                    }
                }
            >
                <option value="">{label}</option>
                {avail.iter().map(|(name, node_idx)| {
                    view! {
                        <option value={node_idx.index().to_string()}>{name.clone()}</option>
                    }
                }).collect::<Vec<_>>()}
            </select>
        </div>
    }.into_view()
}
