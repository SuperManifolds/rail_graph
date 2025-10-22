use leptos::{component, view, IntoView, Children, SignalGet, Signal, store_value};

#[component]
#[must_use]
pub fn ModalOverlay(
    #[prop(into)] is_open: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let children = store_value(children());

    view! {
        {move || if is_open.get() {
            view! {
                <div class="modal-overlay">
                    {children.get_value()}
                </div>
            }.into_view()
        } else {
            ().into_view()
        }}
    }
}
