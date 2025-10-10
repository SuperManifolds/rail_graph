use leptos::{component, view, IntoView, ReadSignal, WriteSignal, SignalGet, SignalSet};

#[derive(Clone, Copy, PartialEq)]
pub enum EditMode {
    None,
    AddingTrack,
    AddingJunction,
}

#[component]
pub fn InfrastructureToolbar(
    auto_layout_enabled: ReadSignal<bool>,
    toggle_auto_layout: impl Fn(()) + 'static,
    set_show_add_station: WriteSignal<bool>,
    edit_mode: ReadSignal<EditMode>,
    set_edit_mode: WriteSignal<EditMode>,
    set_selected_station: WriteSignal<Option<petgraph::graph::NodeIndex>>,
) -> impl IntoView {
    view! {
        <div class="infrastructure-toolbar">
            <button
                class=move || if auto_layout_enabled.get() { "toolbar-button active" } else { "toolbar-button" }
                on:click=move |_| toggle_auto_layout(())
            >
                <i class="fa-solid fa-diagram-project"></i>
                {move || if auto_layout_enabled.get() { " Auto Layout: On" } else { " Auto Layout: Off" }}
            </button>
            <button
                class="toolbar-button"
                on:click=move |_| set_show_add_station.set(true)
            >
                <i class="fa-solid fa-circle-plus"></i>
                " Add Station"
            </button>
            <button
                class=move || if edit_mode.get() == EditMode::AddingTrack { "toolbar-button active" } else { "toolbar-button" }
                on:click=move |_| {
                    if edit_mode.get() == EditMode::AddingTrack {
                        set_edit_mode.set(EditMode::None);
                        set_selected_station.set(None);
                    } else {
                        set_edit_mode.set(EditMode::AddingTrack);
                        set_selected_station.set(None);
                    }
                }
            >
                <i class="fa-solid fa-link"></i>
                " Add Track"
            </button>
            <button
                class=move || if edit_mode.get() == EditMode::AddingJunction { "toolbar-button active" } else { "toolbar-button" }
                on:click=move |_| {
                    if edit_mode.get() == EditMode::AddingJunction {
                        set_edit_mode.set(EditMode::None);
                    } else {
                        set_edit_mode.set(EditMode::AddingJunction);
                    }
                }
            >
                <i class="fa-solid fa-diamond"></i>
                " Add Junction"
            </button>
        </div>
    }
}
