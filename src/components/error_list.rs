use leptos::{component, create_node_ref, create_signal, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, view};
use leptos::leptos_dom::helpers::window_event_listener;
use wasm_bindgen::JsCast;
use crate::conflict::Conflict;
use crate::time::time_to_fraction;
use crate::models::{RailwayGraph, StationNode, Stations};

#[component]
fn ErrorListPopover(
    conflicts: Signal<Vec<Conflict>>,
    on_conflict_click: impl Fn(f64, f64) + 'static + Copy,
    stations: Signal<Vec<(petgraph::stable_graph::NodeIndex, StationNode)>>,
    graph: ReadSignal<RailwayGraph>,
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
                                    let current_stations = stations.get();
                                    current_conflicts.into_iter().map(|conflict| {
                                        let conflict_type_text = conflict.type_name();

                                        // Get station names
                                        let station1_name = current_stations.get(conflict.station1_idx)
                                            .map_or("Unknown", |(_, s)| s.name.as_str());
                                        let station2_name = current_stations.get(conflict.station2_idx)
                                            .map_or("Unknown", |(_, s)| s.name.as_str());

                                        let conflict_message = conflict.format_message(station1_name, station2_name, &graph.get());

                                        let time_fraction = time_to_fraction(conflict.time);
                                        // Direct usize to f64 conversion is safe for reasonable station counts
                                        #[allow(clippy::cast_precision_loss)]
                                        let station_position = conflict.station1_idx as f64 + conflict.position;

                                        view! {
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
                                        }
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
        if !is_open.get() {
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
                    view! {
                        <ErrorListPopover
                            conflicts=conflicts
                            on_conflict_click=on_conflict_click
                            stations=Signal::derive(move || graph.get().get_all_stations_ordered())
                            graph=graph
                        />
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}
