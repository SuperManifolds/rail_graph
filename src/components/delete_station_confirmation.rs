use crate::components::window::Window;
use leptos::{component, IntoView, ReadSignal, Show, Signal, SignalGet, view};
use std::rc::Rc;

#[component]
pub fn DeleteStationConfirmation(
    is_open: ReadSignal<bool>,
    station_name: ReadSignal<String>,
    affected_lines: ReadSignal<Vec<String>>,
    bypass_info: ReadSignal<Option<(String, String)>>, // (from_station, to_station)
    on_cancel: Rc<dyn Fn()>,
    on_confirm: Rc<dyn Fn()>,
) -> impl IntoView {
    let on_cancel_clone = on_cancel.clone();

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Delete Station".to_string())
            on_close=move || on_cancel_clone()
        >
            <div class="delete-confirmation">
                <p class="warning-message">
                    "Are you sure you want to delete station "
                    <strong>{move || station_name.get()}</strong>
                    "?"
                </p>

                <Show
                    when=move || !affected_lines.get().is_empty()
                    fallback=|| view! {
                        <p class="info-message">"This station is not used by any lines."</p>
                    }
                >
                    <div class="affected-lines">
                        <p class="info-message">
                            "This station is part of the following lines:"
                        </p>
                        <ul>
                            {move || affected_lines.get().into_iter().map(|line_name| {
                                view! { <li>{line_name}</li> }
                            }).collect::<Vec<_>>()}
                        </ul>
                        {move || {
                            if let Some((from_station, to_station)) = bypass_info.get() {
                                view! {
                                    <p class="info-message">
                                        "The station will be removed and a direct connection will be created between "
                                        <strong>{from_station}</strong>
                                        " and "
                                        <strong>{to_station}</strong>
                                        " with combined distance and travel times."
                                    </p>
                                }.into_view()
                            } else {
                                view! {
                                    <p class="warning-message">
                                        "This station has multiple connections. Lines passing through will be broken at this point."
                                    </p>
                                }.into_view()
                            }
                        }}
                    </div>
                </Show>

                <div class="form-buttons">
                    <button on:click=move |_| on_cancel()>"Cancel"</button>
                    <button class="danger" on:click=move |_| on_confirm()>"Delete Station"</button>
                </div>
            </div>
        </Window>
    }
}
