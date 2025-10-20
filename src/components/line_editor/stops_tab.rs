use crate::components::tab_view::TabPanel;
use crate::models::{Line, RailwayGraph, RouteDirection, Routes, RouteSegment};
use super::{StopRow, TimeDisplayMode, StationSelect, StationPosition, empty_route_setup::EmptyRouteSetup};
use leptos::*;
use petgraph::stable_graph::NodeIndex;

#[component]
fn PopulatedRoute(
    route_direction: RwSignal<RouteDirection>,
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    time_mode: RwSignal<TimeDisplayMode>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    // Extract route data before the view
    let route_data = create_memo(move |_| {
        edited_line.with(|line| {
            line.as_ref().map(|l| {
                let route = match route_direction.get() {
                    RouteDirection::Forward => &l.forward_route,
                    RouteDirection::Return => &l.return_route,
                };
                route.clone()
            })
        })
    });

    let dir = create_memo(move |_| route_direction.get());

    view! {
        {move || {
            route_data.with(|route_opt| {
                route_opt.as_ref().map(|route| {
                    let stations = graph.with_untracked(|g| {
                        g.get_stations_from_route(route, dir.get_untracked())
                    });

                    view! {
                        <RouteList
                            stations=stations
                            mode=time_mode.get()
                            dir=dir.get()
                            current_route=route.clone()
                            route_direction=route_direction
                            edited_line=edited_line
                            graph=graph
                            on_save=on_save.clone()
                        />
                    }
                })
            })
        }}
    }
}

#[component]
#[allow(clippy::too_many_arguments)]
fn RouteList(
    stations: Vec<(String, NodeIndex)>,
    mode: TimeDisplayMode,
    dir: RouteDirection,
    current_route: Vec<RouteSegment>,
    route_direction: RwSignal<RouteDirection>,
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let column_header = match mode {
        TimeDisplayMode::Difference => "Travel Time to Next",
        TimeDisplayMode::Absolute => "Time from Start",
    };

    let (first_station_idx, last_station_idx, available_start, available_end) = graph.with_untracked(|g| {
        let endpoints = g.get_route_endpoints(&current_route, dir);
        let start = g.get_available_start_stations(&current_route, dir);
        let end = g.get_available_end_stations(&current_route, dir);
        (endpoints.0, endpoints.1, start, end)
    });

    let num_stations = stations.len();
    let stations_with_index: Vec<_> = stations.into_iter().enumerate().collect();

    let on_save_for_start = on_save.clone();
    let on_save_for_list = on_save.clone();
    let on_save_for_end = on_save;

    view! {
        <div class="stops-header">
            <span>"Station"</span>
            <span>"Platform"</span>
            <span>"Track"</span>
            <span>{column_header}</span>
            <span>"Wait Time"</span>
            <span></span>
        </div>

        <StationSelect
            available_stations=available_start
            station_idx=first_station_idx
            position=StationPosition::Start
            route_direction=route_direction.get()
            graph=graph
            edited_line=edited_line
            on_save=on_save_for_start
        />

        <For
            each=move || stations_with_index.clone()
            key=|(_, (_, station_idx))| station_idx.index()
            children=move |(i, (name, station_idx))| {
                view! {
                    <StopRow
                        index=i
                        name=name
                        station_idx=station_idx
                        time_mode=mode
                        route_direction=dir
                        edited_line=edited_line
                        graph=graph
                        on_save=on_save_for_list.clone()
                        is_first={i == 0}
                        is_last={i == num_stations - 1}
                    />
                }
            }
        />

        <StationSelect
            available_stations=available_end
            station_idx=last_station_idx
            position=StationPosition::End
            route_direction=route_direction.get()
            graph=graph
            edited_line=edited_line
            on_save=on_save_for_end
        />
    }
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    active_tab: RwSignal<String>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
    time_mode: RwSignal<TimeDisplayMode>,
    route_direction: RwSignal<RouteDirection>,
    first_station: RwSignal<Option<String>>,
) -> impl IntoView {
    // Store on_save in a reactive context so it can be accessed from closures
    let on_save_stored = store_value(on_save);

    // Memo to check if current route is empty
    let route_is_empty = create_memo(move |_| {
        edited_line.with(|line| {
            line.as_ref().is_none_or(|l| {
                let route = match route_direction.get() {
                    RouteDirection::Forward => &l.forward_route,
                    RouteDirection::Return => &l.return_route,
                };
                route.is_empty()
            })
        })
    });

    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "stops")>
            <div class="line-editor-content">
                <div class="stops-controls">
                    <button
                        class="route-direction-toggle"
                        on:click=move |_| {
                            route_direction.update(|dir| {
                                *dir = match *dir {
                                    RouteDirection::Forward => RouteDirection::Return,
                                    RouteDirection::Return => RouteDirection::Forward,
                                };
                            });
                        }
                        title=move || match route_direction.get() {
                            RouteDirection::Forward => "Switch to return route",
                            RouteDirection::Return => "Switch to forward route",
                        }
                    >
                        {move || match route_direction.get() {
                            RouteDirection::Forward => "→ Forward",
                            RouteDirection::Return => "← Return",
                        }}
                    </button>
                    <button
                        class="time-mode-toggle"
                        on:click=move |_| {
                            time_mode.update(|mode| {
                                *mode = match *mode {
                                    TimeDisplayMode::Difference => TimeDisplayMode::Absolute,
                                    TimeDisplayMode::Absolute => TimeDisplayMode::Difference,
                                };
                            });
                        }
                        title=move || match time_mode.get() {
                            TimeDisplayMode::Difference => "Switch to cumulative time from start",
                            TimeDisplayMode::Absolute => "Switch to time to next stop",
                        }
                    >
                        {move || match time_mode.get() {
                            TimeDisplayMode::Difference => "Δt",
                            TimeDisplayMode::Absolute => "Σt",
                        }}
                    </button>
                    <span class="time-mode-label">
                        {move || match time_mode.get() {
                            TimeDisplayMode::Difference => "Time to next stop",
                            TimeDisplayMode::Absolute => "Cumulative time from start",
                        }}
                    </span>
                </div>
                <div class="stops-list">
                    <Show when=move || edited_line.get().is_some()>
                        <Show
                            when=move || route_is_empty.get()
                            fallback=move || view! {
                                <PopulatedRoute
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    graph=graph
                                    time_mode=time_mode
                                    on_save=on_save_stored.get_value()
                                />
                            }
                        >
                            <EmptyRouteSetup
                                first_station=first_station
                                route_direction=route_direction
                                edited_line=edited_line
                                graph=graph
                                on_save=on_save_stored.get_value()
                            />
                        </Show>
                    </Show>
                </div>
            </div>
        </TabPanel>
    }
}
