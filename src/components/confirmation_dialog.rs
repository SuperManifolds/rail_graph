use leptos::{component, view, Signal, IntoView, SignalGet, create_node_ref, html::Div, create_effect};
use crate::components::window::Window;
use std::rc::Rc;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn ConfirmationDialog(
    is_open: Signal<bool>,
    title: Signal<String>,
    message: Signal<String>,
    on_confirm: Rc<dyn Fn()>,
    on_cancel: Rc<dyn Fn()>,
    #[prop(optional)] confirm_text: Option<String>,
    #[prop(optional)] cancel_text: Option<String>,
) -> impl IntoView {
    let confirm_label = confirm_text.unwrap_or_else(|| "Confirm".to_string());
    let cancel_label = cancel_text.unwrap_or_else(|| "Cancel".to_string());

    let on_cancel_window = on_cancel.clone();
    let on_cancel_button = on_cancel.clone();
    let on_cancel_key = on_cancel.clone();
    let on_confirm_key = on_confirm.clone();

    let div_ref = create_node_ref::<Div>();

    // Focus the div when the dialog opens
    create_effect(move |_| {
        if is_open.get() {
            if let Some(div) = div_ref.get() {
                let _ = div.focus();
            }
        }
    });

    view! {
        <Window
            is_open=is_open
            title=title
            on_close=move || on_cancel_window()
        >
            <div
                class="confirmation-dialog-content"
                tabindex="0"
                node_ref=div_ref
                on:keydown=move |ev| {
                    if ev.key() == "Enter" {
                        on_confirm_key();
                    } else if ev.key() == "Escape" {
                        on_cancel_key();
                    }
                }
            >
                <p class="confirmation-message">
                    {move || message.get()}
                </p>
                <div class="confirmation-buttons">
                    <button
                        class="cancel-button"
                        on:click=move |_| on_cancel_button()
                    >
                        {cancel_label.clone()}
                    </button>
                    <button
                        class="confirm-button danger"
                        on:click=move |_| on_confirm()
                    >
                        {confirm_label.clone()}
                    </button>
                </div>
            </div>
        </Window>
    }
}
