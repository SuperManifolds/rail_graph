use leptos::{component, view, Signal, IntoView, ReadSignal, WriteSignal, SignalSet, event_target_value};
use crate::components::window::Window;
use std::rc::Rc;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn TextInputDialog(
    is_open: Signal<bool>,
    title: Signal<String>,
    label: String,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    on_confirm: Rc<dyn Fn()>,
    on_cancel: Rc<dyn Fn()>,
    #[prop(optional)] confirm_text: Option<String>,
    #[prop(optional)] cancel_text: Option<String>,
) -> impl IntoView {
    let confirm_label = confirm_text.unwrap_or_else(|| "Confirm".to_string());
    let cancel_label = cancel_text.unwrap_or_else(|| "Cancel".to_string());

    let on_cancel_window = on_cancel.clone();
    let on_cancel_button = on_cancel.clone();
    let on_confirm_enter = on_confirm.clone();
    let on_confirm_button = on_confirm.clone();

    view! {
        <Window
            is_open=is_open
            title=title
            on_close=move || on_cancel_window()
            max_size=(400.0, 200.0)
        >
            <div class="save-as-dialog">
                <label>{label}</label>
                <input
                    type="text"
                    class="project-name-input"
                    value=value
                    on:input=move |ev| set_value.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            on_confirm_enter();
                        }
                    }
                    prop:autofocus=true
                />
                <div class="dialog-buttons">
                    <button on:click=move |_| on_cancel_button()>
                        {cancel_label.clone()}
                    </button>
                    <button class="primary" on:click=move |_| on_confirm_button()>
                        {confirm_label.clone()}
                    </button>
                </div>
            </div>
        </Window>
    }
}
