use crate::components::window::Window;
use crate::models::RailwayGraph;
use leptos::*;
use petgraph::graph::NodeIndex;
use std::rc::Rc;

#[component]
pub fn EditStation(
    editing_station: ReadSignal<Option<NodeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: Rc<dyn Fn(NodeIndex, String, bool)>,
    on_delete: Rc<dyn Fn(NodeIndex)>,
    graph: ReadSignal<RailwayGraph>,
) -> impl IntoView {
    let (station_name, set_station_name) = create_signal(String::new());
    let (is_passing_loop, set_is_passing_loop) = create_signal(false);

    // Load current station data when dialog opens
    create_effect(move |_| {
        if let Some(idx) = editing_station.get() {
            let current_graph = graph.get();
            if let Some(station) = current_graph.graph.node_weight(idx) {
                set_station_name.set(station.name.clone());
                set_is_passing_loop.set(station.passing_loop);
            }
        }
    });

    let on_close_clone = on_close.clone();
    let handle_save = move |_| {
        if let Some(idx) = editing_station.get() {
            let name = station_name.get();
            if !name.is_empty() {
                on_save(idx, name, is_passing_loop.get());
            }
        }
    };

    let handle_delete = move |_| {
        if let Some(idx) = editing_station.get() {
            on_delete(idx);
        }
    };

    let is_open = Signal::derive(move || editing_station.get().is_some());

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Edit Station".to_string())
            on_close=move || on_close_clone()
        >
            <div class="add-station-form">
                <div class="form-field">
                    <label>"Station Name"</label>
                    <input
                        type="text"
                        value=move || station_name.get()
                        on:input=move |ev| set_station_name.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-field">
                    <label>
                        <input
                            type="checkbox"
                            checked=move || is_passing_loop.get()
                            on:change=move |ev| set_is_passing_loop.set(event_target_checked(&ev))
                        />
                        " Passing Loop"
                    </label>
                </div>
                <div class="form-buttons">
                    <button class="danger" on:click=handle_delete>"Delete"</button>
                    <div style="flex: 1;"></div>
                    <button on:click=move |_| on_close()>"Cancel"</button>
                    <button class="primary" on:click=handle_save>"Save"</button>
                </div>
            </div>
        </Window>
    }
}
