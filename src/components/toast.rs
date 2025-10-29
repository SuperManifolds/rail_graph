use leptos::{component, view, IntoView, ReadSignal, SignalGet};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Toast {
    pub message: String,
    pub visible: bool,
}

impl Toast {
    #[must_use]
    pub fn new(message: String) -> Self {
        Self {
            message,
            visible: true,
        }
    }
}

#[component]
#[must_use]
pub fn ToastNotification(toast: ReadSignal<Toast>) -> impl IntoView {
    view! {
        {move || {
            let t = toast.get();
            if t.visible {
                view! {
                    <div class="toast toast-visible">
                        {t.message}
                    </div>
                }.into_view()
            } else {
                view! { <div class="toast"></div> }.into_view()
            }
        }}
    }
}
