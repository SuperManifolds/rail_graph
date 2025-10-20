mod general_tab;
mod stops_tab;
mod schedule_tab;
mod auto_schedule_form;
mod manual_departure_editor;
mod manual_departures_list;
mod platform_select;
mod stop_row;
mod station_select;
mod empty_route_setup;

pub use general_tab::GeneralTab;
pub use stops_tab::StopsTab;
pub use schedule_tab::ScheduleTab;
pub use manual_departure_editor::ManualDepartureEditor;
pub use manual_departures_list::ManualDeparturesList;
pub use platform_select::{PlatformSelect, PlatformField};
pub use stop_row::{StopRow, TimeDisplayMode};
pub use station_select::{StationSelect, StationPosition};

use crate::components::{tab_view::{Tab, TabView}, window::Window};
use crate::models::{Line, RailwayGraph, RouteDirection};
use leptos::{component, view, MaybeSignal, Signal, ReadSignal, IntoView, create_signal, create_rw_signal, create_effect, SignalGet, SignalGetUntracked, SignalSet, store_value, Show};
use std::rc::Rc;

#[component]
pub fn LineEditor(
    #[prop(into)] initial_line: MaybeSignal<Option<Line>>,
    is_open: Signal<bool>,
    set_is_open: impl Fn(bool) + 'static,
    graph: ReadSignal<RailwayGraph>,
    on_save: impl Fn(Line) + 'static,
) -> impl IntoView {
    let (edited_line, set_edited_line) = create_signal(None::<Line>);
    let active_tab = create_rw_signal("general".to_string());

    // Persistent UI state for StopsTab to avoid resets on save
    let time_mode = create_rw_signal(TimeDisplayMode::Difference);
    let route_direction = create_rw_signal(RouteDirection::Forward);
    let first_station = create_rw_signal(None::<String>);

    // Reset edited_line when dialog opens (not when initial_line changes)
    create_effect(move |prev_open| {
        let currently_open = is_open.get();
        if currently_open && prev_open != Some(true) {
            if let Some(line) = initial_line.get_untracked() {
                set_edited_line.set(Some(line));
            }
        }
        currently_open
    });

    // Wrap on_save to also update local edited_line state
    let on_save_wrapped = Rc::new(move |line: Line| {
        set_edited_line.set(Some(line.clone()));
        on_save(line);
    });
    let set_is_open = store_value(set_is_open);

    let close_dialog = move || {
        set_is_open.with_value(|f| f(false));
    };

    let window_title = Signal::derive(move || {
        edited_line
            .get()
            .map_or_else(|| "Edit Line".to_string(), |line| format!("Edit Line: {}", line.id))
    });

    let is_window_open = Signal::derive(move || is_open.get() && edited_line.get().is_some());

    let on_save_stored = store_value(on_save_wrapped);
    let tabs = store_value(vec![
        Tab { id: "general".to_string(), label: "General".to_string() },
        Tab { id: "stops".to_string(), label: "Stops".to_string() },
        Tab { id: "schedule".to_string(), label: "Schedule".to_string() },
    ]);

    view! {
        <Window
            is_open=is_window_open
            title=window_title
            on_close=close_dialog
            max_size=(900.0, 1200.0)
        >
            <Show when=move || edited_line.get().is_some()>
                <TabView tabs=tabs.get_value() active_tab=active_tab>
                    <GeneralTab
                        edited_line=edited_line
                        set_edited_line=set_edited_line
                        on_save=on_save_stored.get_value()
                        active_tab=active_tab
                    />
                    <StopsTab
                        edited_line=edited_line
                        graph=graph
                        active_tab=active_tab
                        on_save=on_save_stored.get_value()
                        time_mode=time_mode
                        route_direction=route_direction
                        first_station=first_station
                    />
                    <ScheduleTab
                        edited_line=edited_line
                        set_edited_line=set_edited_line
                        graph=graph
                        on_save=on_save_stored.get_value()
                        active_tab=active_tab
                    />
                </TabView>
            </Show>
        </Window>
    }
}
