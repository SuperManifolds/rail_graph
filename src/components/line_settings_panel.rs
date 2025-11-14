use leptos::{component, view, IntoView, ReadSignal, WriteSignal, SignalGet, SignalSet, event_target_checked};

#[component]
#[must_use]
pub fn LineSettingsPanel(
    /// Whether line mode is active (panel only shows when true)
    show_lines: ReadSignal<bool>,
    /// Whether to hide unscheduled stations and tracks in line mode
    hide_unscheduled: ReadSignal<bool>,
    /// Setter for hide_unscheduled
    set_hide_unscheduled: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        <div
            class="line-settings-panel"
            class:hidden=move || !show_lines.get()
        >
            <label class="line-settings-checkbox">
                <input
                    type="checkbox"
                    checked=move || hide_unscheduled.get()
                    on:change=move |ev| {
                        let checked = event_target_checked(&ev);
                        set_hide_unscheduled.set(checked);
                    }
                />
                <span>"Hide unscheduled stations and tracks"</span>
            </label>
        </div>
    }
}
