use crate::components::line_editor::{GeneralTab, ScheduleTab, StopsTab, TimeDisplayMode};
use crate::components::tab_view::{Tab, TabView};
use crate::models::RouteDirection;
use crate::window_protocol::{LineEditorInit, LineEditorResult};
use leptos::{
    component, create_rw_signal, create_signal, store_value, view, IntoView, Show, SignalGet,
    SignalSet,
};
use std::rc::Rc;

#[component]
#[must_use]
pub fn LineEditorChild(init: LineEditorInit, session: String) -> impl IntoView {
    let initial_tab = init.initial_tab.unwrap_or_else(|| "general".to_string());
    let (edited_line, set_edited_line) = create_signal(Some(init.line));
    let (graph, _) = create_signal(init.graph);
    let (settings, _) = create_signal(init.settings);
    let active_tab = create_rw_signal(initial_tab);

    // Persistent UI state for StopsTab
    let time_mode = create_rw_signal(TimeDisplayMode::Difference);
    let route_direction = create_rw_signal(RouteDirection::Forward);
    let first_station = create_rw_signal(None::<String>);

    let session_save = session;
    let on_save: Rc<dyn Fn(crate::models::Line)> = Rc::new(move |line: crate::models::Line| {
        set_edited_line.set(Some(line.clone()));
        let result = LineEditorResult::Save(line);
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_save}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    });

    let on_save_stored = store_value(on_save);
    let tabs = store_value(vec![
        Tab {
            id: "general".to_string(),
            label: "General".to_string(),
        },
        Tab {
            id: "stops".to_string(),
            label: "Stops".to_string(),
        },
        Tab {
            id: "schedule".to_string(),
            label: "Schedule".to_string(),
        },
    ]);

    view! {
        <div class="child-window-content">
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
                        settings=settings
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
        </div>
    }
}
