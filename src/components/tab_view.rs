use leptos::*;

#[derive(Clone, PartialEq)]
pub struct Tab {
    pub id: String,
    pub label: String,
}

#[component]
pub fn TabView(
    tabs: Vec<Tab>,
    #[prop(into)] active_tab: RwSignal<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="tab-view">
            <div class="tab-header">
                {tabs.into_iter().map(|tab| {
                    let tab_id = tab.id.clone();
                    let tab_id_for_click = tab.id.clone();
                    view! {
                        <button
                            class=move || {
                                if active_tab.get() == tab_id {
                                    "tab-button active"
                                } else {
                                    "tab-button"
                                }
                            }
                            on:click=move |_| active_tab.set(tab_id_for_click.clone())
                        >
                            {tab.label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
            <div class="tab-content">
                {children()}
            </div>
        </div>
    }
}

#[component]
pub fn TabPanel(
    when: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let children = store_value(children());
    view! {
        <Show when=move || when.get()>
            {children.with_value(|c| c.clone())}
        </Show>
    }
}
