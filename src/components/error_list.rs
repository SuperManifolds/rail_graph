use leptos::{component, create_node_ref, create_signal, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, view};
use leptos::leptos_dom::helpers::window_event_listener;
use wasm_bindgen::JsCast;
use crate::conflict::Conflict;
use crate::time::time_to_fraction;
use crate::models::{RailwayGraph, Node, Stations};

#[component]
fn ErrorListPopover(
    conflicts: Signal<Vec<Conflict>>,
    on_conflict_click: impl Fn(f64, f64) + 'static + Copy,
    nodes: Signal<Vec<(petgraph::stable_graph::NodeIndex, Node)>>,
    graph: ReadSignal<RailwayGraph>,
    station_idx_map: Signal<std::collections::HashMap<usize, usize>>,
) -> impl IntoView {
    view! {
        <div class="error-list-popover">
            <div class="error-list-content">
                {move || {
                    let current_conflicts = conflicts.get();
                    if current_conflicts.is_empty() {
                        view! {
                            <p class="no-errors">"No conflicts detected"</p>
                        }.into_view()
                    } else {
                        view! {
                            <div class="error-items">
                                {
                                    let current_nodes = nodes.get();
                                    let idx_map = station_idx_map.get();
                                    current_conflicts.into_iter().filter_map(|conflict| {
                                        let conflict_type_text = conflict.type_name();

                                        // Map conflict indices to display indices
                                        let display_idx1 = *idx_map.get(&conflict.station1_idx)?;
                                        let display_idx2 = *idx_map.get(&conflict.station2_idx)?;

                                        // Get node names
                                        let station1_name = current_nodes.get(display_idx1)
                                            .map_or_else(|| "Unknown".to_string(), |(_, n)| n.display_name().to_string());
                                        let station2_name = current_nodes.get(display_idx2)
                                            .map_or_else(|| "Unknown".to_string(), |(_, n)| n.display_name().to_string());

                                        let conflict_message = if conflict.conflict_type == crate::conflict::ConflictType::PlatformViolation {
                                            // Look up platform name directly from nodes to avoid expensive graph traversal
                                            let platform_name = conflict.platform_idx.and_then(|idx| {
                                                current_nodes.get(display_idx1)
                                                    .and_then(|(_, n)| n.as_station())
                                                    .and_then(|s| s.platforms.get(idx))
                                                    .map(|p| p.name.as_str())
                                            }).unwrap_or("?");
                                            conflict.format_platform_message(&station1_name, platform_name)
                                        } else {
                                            conflict.format_message(&station1_name, &station2_name, &graph.get())
                                        };

                                        let time_fraction = time_to_fraction(conflict.time);
                                        // Calculate display position using mapped indices
                                        #[allow(clippy::cast_precision_loss)]
                                        let station_position = display_idx1 as f64 + (conflict.position * (display_idx2 as f64 - display_idx1 as f64));

                                        Some(view! {
                                            <div
                                                class="error-item clickable"
                                                on:click=move |_| {
                                                    on_conflict_click(time_fraction, station_position);
                                                }
                                            >
                                                <div class="error-item-header">
                                                    <i class="fa-solid fa-triangle-exclamation"></i>
                                                    <span class="error-type">{conflict_type_text}</span>
                                                </div>
                                                <div class="error-item-details">
                                                    <div class="error-detail">
                                                        <span class="value">{conflict_message}</span>
                                                    </div>
                                                    <div class="error-detail">
                                                        <span class="value">{conflict.time.format("%H:%M:%S").to_string()}</span>
                                                    </div>
                                                </div>
                                            </div>
                                        })
                                    }).collect::<Vec<_>>()
                                }
                            </div>
                        }.into_view()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn ErrorList(
    conflicts: Signal<Vec<Conflict>>,
    on_conflict_click: impl Fn(f64, f64) + 'static + Copy,
    graph: ReadSignal<RailwayGraph>,
    station_idx_map: Signal<std::collections::HashMap<usize, usize>>,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);

    let toggle_popover = move |_| {
        set_is_open.update(|open| *open = !*open);
    };

    let conflict_count = move || conflicts.get().len();
    let has_errors = move || conflict_count() > 0;

    // Close when clicking outside
    let container_ref = create_node_ref::<leptos::html::Div>();

    window_event_listener(leptos::ev::click, move |ev| {
        // Use try_get to avoid panic if signal is disposed
        if is_open.try_get() != Some(true) {
            return;
        }
        let Some(container) = container_ref.get() else {
            return;
        };
        let target = ev.target();
        let Some(target_element) = target.and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else {
            return;
        };
        if !container.contains(Some(&target_element)) {
            set_is_open.set(false);
        }
    });

    view! {
        <div class="error-list-container" node_ref=container_ref>
            {move || {
                if has_errors() {
                    view! {
                        <button
                            class="error-list-button has-errors"
                            on:click=toggle_popover
                        >
                            <i class="fa-solid fa-triangle-exclamation"></i>
                            <span class="error-count">{conflict_count()}</span>
                            <span class="error-label">" Conflicts"</span>
                        </button>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            {move || {
                if is_open.get() {
                    // Get nodes once when opening dialog to avoid repeated expensive calls
                    let all_nodes = graph.get().get_all_nodes_ordered();
                    let nodes_signal = Signal::derive(move || all_nodes.clone());
                    view! {
                        <ErrorListPopover
                            conflicts=conflicts
                            on_conflict_click=on_conflict_click
                            nodes=nodes_signal
                            graph=graph
                            station_idx_map=station_idx_map
                        />
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}
