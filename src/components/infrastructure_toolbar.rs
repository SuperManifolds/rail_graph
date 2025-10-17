use leptos::{component, view, IntoView, ReadSignal, WriteSignal, SignalGet, SignalSet, Callback, Signal};
use petgraph::stable_graph::NodeIndex;
use crate::components::button::Button;

#[derive(Clone, Copy, PartialEq)]
pub enum EditMode {
    None,
    AddingTrack,
    AddingJunction,
    CreatingView,
}

#[component]
pub fn InfrastructureToolbar(
    auto_layout_enabled: ReadSignal<bool>,
    toggle_auto_layout: impl Fn(()) + 'static,
    set_show_add_station: WriteSignal<bool>,
    edit_mode: ReadSignal<EditMode>,
    set_edit_mode: WriteSignal<EditMode>,
    set_selected_station: WriteSignal<Option<NodeIndex>>,
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
            <Button
                class="toolbar-button"
                on_click=Callback::new(move |_| set_show_add_station.set(true))
                shortcut="S"
                title="Add Station"
            >
                <i class="fa-solid fa-circle-plus"></i>
                " Add Station"
            </Button>
            <Button
                class="toolbar-button"
                active=Signal::derive(move || edit_mode.get() == EditMode::AddingTrack)
                on_click=Callback::new(move |_| {
                    if edit_mode.get() == EditMode::AddingTrack {
                        set_edit_mode.set(EditMode::None);
                        set_selected_station.set(None);
                    } else {
                        set_edit_mode.set(EditMode::AddingTrack);
                        set_selected_station.set(None);
                    }
                })
                shortcut="T"
                title="Add Track"
            >
                <i class="fa-solid fa-link"></i>
                " Add Track"
            </Button>
            <Button
                class="toolbar-button"
                active=Signal::derive(move || edit_mode.get() == EditMode::AddingJunction)
                on_click=Callback::new(move |_| {
                    if edit_mode.get() == EditMode::AddingJunction {
                        set_edit_mode.set(EditMode::None);
                    } else {
                        set_edit_mode.set(EditMode::AddingJunction);
                    }
                })
                shortcut="J"
                title="Add Junction"
            >
                <i class="fa-solid fa-diamond"></i>
                " Add Junction"
            </Button>
            <Button
                class="toolbar-button"
                active=Signal::derive(move || edit_mode.get() == EditMode::CreatingView)
                on_click=Callback::new(move |_| {
                    if edit_mode.get() == EditMode::CreatingView {
                        set_edit_mode.set(EditMode::None);
                    } else {
                        set_edit_mode.set(EditMode::CreatingView);
                    }
                })
                shortcut="N"
                title="Create View"
            >
                <i class="fa-solid fa-eye"></i>
                " Create View"
            </Button>
        </div>
    }
}
