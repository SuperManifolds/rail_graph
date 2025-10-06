use crate::components::window::Window;
use leptos::*;
use std::rc::Rc;

#[component]
pub fn DeleteStationConfirmation(
    is_open: ReadSignal<bool>,
    station_name: ReadSignal<String>,
    affected_lines: ReadSignal<Vec<String>>,
    on_cancel: Rc<dyn Fn()>,
    on_confirm: Rc<dyn Fn()>,
) -> impl IntoView {
    let on_cancel_clone = on_cancel.clone();

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Delete Station".to_string())
            on_close=move || on_cancel_clone()
            initial_size=(450.0, 300.0)
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
                        <p class="info-message">
                            "The station will be removed from these lines, and adjacent stops will be connected directly with combined travel times."
                        </p>
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
