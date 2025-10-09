use leptos::*;

#[derive(Clone, PartialEq)]
pub struct Tab {
    pub id: String,
    pub label: String,
}

#[component]
#[must_use]
pub fn TabView(
    tabs: Vec<Tab>,
    #[prop(into)] active_tab: RwSignal<String>,
    children: Children,
) -> impl IntoView {
    // Try to get the window resize trigger from context
    let maybe_trigger_resize = use_context::<WriteSignal<u32>>();

    // Watch for tab changes and trigger window resize
    create_effect(move |_| {
        let _ = active_tab.get(); // Track tab changes
        if let Some(trigger) = maybe_trigger_resize {
            // Increment the trigger to signal a resize
            trigger.update(|v| *v = v.wrapping_add(1));
        }
    });

    view! {
        <div class="tab-view">
            <div class="tab-header">
                {tabs.into_iter().map(|tab| {
                    let tab_id = tab.id.clone();
                    let tab_class = move || {
                        if active_tab.get() == tab_id {
                            "tab-button active"
                        } else {
                            "tab-button"
                        }
                    };
                    view! {
                        <button class=tab_class on:click=move |_| active_tab.set(tab.id.clone())>
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
#[must_use]
pub fn TabPanel(when: Signal<bool>, children: Children) -> impl IntoView {
    let children = store_value(children());
    view! {
        <Show when=move || when.get()>
            {children.with_value(|c| c.clone())}
        </Show>
    }
}
