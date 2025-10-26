use leptos::{component, view, IntoView, ReadSignal, WriteSignal, SignalWith, SignalUpdate};
use crate::models::{LineSortMode, ProjectSettings};

#[component]
#[must_use]
pub fn LineSortSelector(
    settings: ReadSignal<ProjectSettings>,
    set_settings: WriteSignal<ProjectSettings>,
) -> impl IntoView {
    view! {
        <div class="sort-mode-selector">
            <button
                class=move || if settings.with(|s| s.line_sort_mode == LineSortMode::AddedOrder) { "active" } else { "" }
                on:click=move |_| {
                    set_settings.update(|s| s.line_sort_mode = LineSortMode::AddedOrder);
                }
                title="Added Order"
            >
                <i class="fa-solid fa-list-ol"></i>
            </button>
            <button
                class=move || if settings.with(|s| s.line_sort_mode == LineSortMode::Alphabetical) { "active" } else { "" }
                on:click=move |_| {
                    set_settings.update(|s| s.line_sort_mode = LineSortMode::Alphabetical);
                }
                title="Alphabetical"
            >
                <i class="fa-solid fa-arrow-down-a-z"></i>
            </button>
            <button
                class=move || if settings.with(|s| s.line_sort_mode == LineSortMode::Manual) { "active" } else { "" }
                on:click=move |_| {
                    set_settings.update(|s| s.line_sort_mode = LineSortMode::Manual);
                }
                title="Manual"
            >
                <i class="fa-solid fa-grip-vertical"></i>
            </button>
        </div>
    }
}
