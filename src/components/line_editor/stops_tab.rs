use crate::components::tab_view::TabPanel;
use crate::models::{Line, RailwayGraph, RouteDirection, Routes};
use super::{StopRow, TimeDisplayMode, StationSelect, StationPosition};
use leptos::*;

#[component]
pub fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    active_tab: RwSignal<String>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let (time_mode, set_time_mode) = create_signal(TimeDisplayMode::Difference);
    let (route_direction, set_route_direction) = create_signal(RouteDirection::Forward);
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "stops")>
            <div class="line-editor-content">
                <div class="stops-controls">
                    <button
                        class="route-direction-toggle"
                        on:click=move |_| {
                            set_route_direction.update(|dir| {
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
                            set_time_mode.update(|mode| {
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
                    {move || {
                        edited_line.get().map(|line| {
                            let current_graph = graph.get();

                            let current_route = match route_direction.get() {
                                RouteDirection::Forward => &line.forward_route,
                                RouteDirection::Return => &line.return_route,
                            };

                            if current_route.is_empty() {
                                view! {
                                    <p class="no-stops">"No stops defined for this route yet. Import a CSV to set up the route."</p>
                                }.into_view()
                            } else {
                                let stations = current_graph.get_stations_from_route(current_route, route_direction.get());

                                let mode = time_mode.get();
                                let dir = route_direction.get();
                                let column_header = match mode {
                                    TimeDisplayMode::Difference => "Travel Time to Next",
                                    TimeDisplayMode::Absolute => "Time from Start",
                                };

                                let (first_station_idx, last_station_idx) = current_graph.get_route_endpoints(current_route, dir);
                                let available_start = current_graph.get_available_start_stations(current_route, dir);
                                let available_end = current_graph.get_available_end_stations(current_route, dir);

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
                                        on_save=on_save.clone()
                                    />

                                    {
                                        let num_stations = stations.len();
                                        stations.into_iter().enumerate().map(|(i, (name, station_idx))| {
                                            view! {
                                                <StopRow
                                                    index=i
                                                    name=name
                                                    station_idx=station_idx
                                                    line=line.clone()
                                                    graph=current_graph.clone()
                                                    time_mode=mode
                                                    route_direction=dir
                                                    edited_line=edited_line
                                                    on_save=on_save.clone()
                                                    is_first={i == 0}
                                                    is_last={i == num_stations - 1}
                                                />
                                            }
                                        }).collect::<Vec<_>>()
                                    }

                                    <StationSelect
                                        available_stations=available_end
                                        station_idx=last_station_idx
                                        position=StationPosition::End
                                        route_direction=route_direction.get()
                                        graph=graph
                                        edited_line=edited_line
                                        on_save=on_save.clone()
                                    />
                                }.into_view()
                            }
                        })
                    }}
                </div>
            </div>
        </TabPanel>
    }
}
