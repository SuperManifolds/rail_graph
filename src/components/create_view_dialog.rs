use leptos::{component, view, IntoView, ReadSignal, Signal, SignalGet, SignalSet, create_signal, event_target_value};
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;
use crate::models::{RailwayGraph, Stations};
use crate::components::window::Window;

#[component]
pub fn CreateViewDialog(
    is_open: ReadSignal<bool>,
    start_station: ReadSignal<Option<NodeIndex>>,
    end_station: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    on_close: Rc<dyn Fn()>,
    on_create: Rc<dyn Fn(String, NodeIndex, NodeIndex)>,
) -> impl IntoView {
    let (view_name, set_view_name) = create_signal(String::new());

    let get_station_name = move |idx: Option<NodeIndex>| {
        idx.and_then(|i| graph.get().get_station_name(i).map(String::from))
            .unwrap_or_default()
    };

    let on_create_clone = on_create.clone();
    let on_create_clone2 = on_create.clone();
    let handle_create = move |()| {
        if let (Some(start), Some(end)) = (start_station.get(), end_station.get()) {
            if !view_name.get().trim().is_empty() {
                on_create_clone(view_name.get().trim().to_string(), start, end);
                set_view_name.set(String::new());
            }
        }
    };

    let handle_create2 = move |()| {
        web_sys::console::log_1(&"Create button clicked".into());
        web_sys::console::log_1(&format!("Start: {:?}, End: {:?}", start_station.get(), end_station.get()).into());
        web_sys::console::log_1(&format!("Name: {}", view_name.get()).into());

        if let (Some(start), Some(end)) = (start_station.get(), end_station.get()) {
            if view_name.get().trim().is_empty() {
                web_sys::console::log_1(&"View name is empty".into());
            } else {
                web_sys::console::log_1(&"Calling on_create".into());
                on_create_clone2(view_name.get().trim().to_string(), start, end);
                set_view_name.set(String::new());
            }
        } else {
            web_sys::console::log_1(&"Start or end station is None".into());
        }
    };

    let on_close_clone = on_close.clone();

    view! {
        <Window
            is_open=leptos::MaybeSignal::Dynamic(is_open.into())
            title=Signal::derive(|| "Create View".to_string())
            on_close=move || {
                set_view_name.set(String::new());
                on_close_clone();
            }
        >
            <div class="add-station-form">
                <div class="form-field">
                    <label>"View Name"</label>
                    <input
                        type="text"
                        placeholder="Enter view name"
                        value=view_name
                        on:input=move |ev| set_view_name.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                handle_create(());
                            }
                        }
                        prop:autofocus=true
                    />
                </div>
                <div class="form-field">
                    <label>"From Station"</label>
                    <div class="station-name-display">{move || get_station_name(start_station.get())}</div>
                </div>
                <div class="form-field">
                    <label>"To Station"</label>
                    <div class="station-name-display">{move || get_station_name(end_station.get())}</div>
                </div>
                <div class="form-buttons">
                    <button on:click=move |_| on_close()>"Cancel"</button>
                    <button
                        class="primary"
                        on:click=move |_| handle_create2(())
                        prop:disabled=move || view_name.get().trim().is_empty()
                    >
                        "Create View"
                    </button>
                </div>
            </div>
        </Window>
    }
}
