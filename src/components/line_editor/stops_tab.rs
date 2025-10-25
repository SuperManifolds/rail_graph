use super::{
    empty_route_setup::EmptyRouteSetup, StationPosition, StationSelect, StopRow, TimeDisplayMode,
};
use crate::components::tab_view::TabPanel;
use crate::models::{Line, RailwayGraph, RouteDirection, Routes};
use leptos::*;

fn get_column_header(mode: TimeDisplayMode) -> &'static str {
    match mode {
        TimeDisplayMode::Difference => "Travel Time to Next",
        TimeDisplayMode::Absolute => "Time from Start",
    }
}


#[component]
fn RouteStopsList(
    route_direction: RwSignal<RouteDirection>,
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    time_mode: RwSignal<TimeDisplayMode>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
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

    let stations_data = create_memo(move |_| {
        route_data.with(|route_opt| {
            route_opt.as_ref().map(|route| {
                graph.with_untracked(|g| g.get_stations_from_route(route, dir.get_untracked()))
            })
        })
    });

    let endpoints = create_memo(move |_| {
        route_data.with(|route_opt| {
            route_opt.as_ref().map(|route| {
                graph.with_untracked(|g| g.get_route_endpoints(route, dir.get()))
            })
        })
    });

    let available_start = create_memo(move |_| {
        route_data.with(|route_opt| {
            route_opt.as_ref().map(|route| {
                graph.with_untracked(|g| g.get_available_start_stations(route, dir.get()))
            })
        })
    });

    let available_end = create_memo(move |_| {
        route_data.with(|route_opt| {
            route_opt.as_ref().map(|route| {
                graph.with_untracked(|g| g.get_available_end_stations(route, dir.get()))
            })
        })
    });

    let on_save_stored = store_value(on_save);

    let on_save = on_save_stored.get_value();
    let on_save_for_start = on_save.clone();
    let on_save_for_list = on_save.clone();
    let on_save_for_end = on_save;

    view! {
        <div class="stops-header">
            <span>"Station"</span>
            <span>"Platform"</span>
            <span>"Track"</span>
            <span>{move || get_column_header(time_mode.get())}</span>
            <span>"Wait Time"</span>
            <span></span>
        </div>

        {move || {
            let eps = endpoints.get()?;
            let avail = available_start.get()?;
            let current_dir = dir.get();
            Some(view! {
                <StationSelect
                    available_stations=avail
                    station_idx=eps.0
                    position=StationPosition::Start
                    route_direction=current_dir
                    graph=graph
                    edited_line=edited_line
                    on_save=on_save_for_start.clone()
                />
            })
        }}

        <For
            each=move || {
                let current_dir = dir.get();
                let current_mode = time_mode.get();
                stations_data.get().map(|stations| {
                    let num_stations = stations.len();
                    stations.into_iter().enumerate().map(|(i, (name, station_idx))| {
                        let is_first = i == 0;
                        let is_last = i == num_stations - 1;
                        (i, name, station_idx, current_dir, current_mode, is_first, is_last)
                    }).collect::<Vec<_>>()
                }).unwrap_or_default()
            }
            key=|(i, _, station_idx, current_dir, current_mode, is_first, is_last)| (station_idx.index(), *i, *current_dir as u8, *current_mode as u8, *is_first, *is_last)
            children=move |(i, name, station_idx, current_dir, current_mode, is_first, is_last)| {
                view! {
                    <StopRow
                        index=i
                        name=name
                        station_idx=station_idx
                        time_mode=current_mode
                        route_direction=current_dir
                        edited_line=edited_line
                        graph=graph
                        on_save=on_save_for_list.clone()
                        is_first=is_first
                        is_last=is_last
                    />
                }
            }
        />

        {move || {
            let eps = endpoints.get()?;
            let avail = available_end.get()?;
            let current_dir = dir.get();
            Some(view! {
                <StationSelect
                    available_stations=avail
                    station_idx=eps.1
                    position=StationPosition::End
                    route_direction=current_dir
                    graph=graph
                    edited_line=edited_line
                    on_save=on_save_for_end.clone()
                />
            })
        }}
    }
    .into_view()
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
    settings: ReadSignal<crate::models::ProjectSettings>,
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
                    <Show
                        when=move || route_is_empty.get()
                        fallback=move || view! {
                            <RouteStopsList
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
                            settings=settings
                        />
                    </Show>
                </div>
            </div>
        </TabPanel>
    }
}
