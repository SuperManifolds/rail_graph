mod auto_schedule_form;
mod empty_route_setup;
mod general_tab;
mod manual_departure_editor;
mod manual_departures_list;
mod platform_column;
mod platform_select;
mod schedule_tab;
mod station_select;
mod stop_row;
mod stops_tab;
mod time_column;
mod track_column;
mod wait_time_column;

pub use general_tab::GeneralTab;
pub use manual_departure_editor::ManualDepartureEditor;
pub use manual_departures_list::ManualDeparturesList;
pub use platform_column::PlatformColumn;
pub use platform_select::{PlatformField, PlatformSelect};
pub use schedule_tab::ScheduleTab;
pub use station_select::StationSelect;
pub use stop_row::StopRow;
pub use stops_tab::StopsTab;
pub use time_column::{TimeColumn, TimeDisplayMode};
pub use track_column::TrackColumn;
pub use wait_time_column::WaitTimeColumn;
pub use crate::models::StationPosition;

use crate::components::native_window::NativeWindow;
use crate::models::{Line, RailwayGraph};
use crate::window_protocol::{LineEditorInit, LineEditorResult};
use leptos::{
    component, store_value, view, IntoView, MaybeSignal, ReadSignal, Signal, SignalGet,
};

#[component]
pub fn LineEditor(
    #[prop(into)] initial_line: MaybeSignal<Option<Line>>,
    is_open: Signal<bool>,
    set_is_open: impl Fn(bool) + 'static,
    graph: ReadSignal<RailwayGraph>,
    on_save: impl Fn(Line) + 'static,
    settings: ReadSignal<crate::models::ProjectSettings>,
    #[prop(default = None)] initial_tab: Option<String>,
) -> impl IntoView {
    let set_is_open = store_value(set_is_open);
    let initial_tab = store_value(initial_tab);

    let on_close_for_window = move || {
        set_is_open.with_value(|f| f(false));
    };

    let on_close_for_result = on_close_for_window;

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<LineEditorResult>(&json) {
            Ok(LineEditorResult::Save(line)) => {
                on_save(line);
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse LineEditorResult: {}", e);
            }
        }
        on_close_for_result();
    });

    let initial_line_for_init = initial_line.clone();
    let window_title = Signal::derive(move || {
        initial_line.get().map_or_else(
            || "Edit Line".to_string(),
            |line| format!("Edit Line: {}", line.name),
        )
    });

    view! {
        <NativeWindow
            is_open=is_open
            title=window_title
            on_close=move || on_close_for_window()
            window_type="line-editor"
            init_data=Signal::derive(move || {
                let Some(line) = initial_line_for_init.get() else {
                    return String::new();
                };
                serde_json::to_string(&LineEditorInit {
                    line,
                    graph: graph.get(),
                    settings: settings.get(),
                    initial_tab: initial_tab.get_value(),
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(900, 700)
            position_key="line-editor"
        />
    }
}
