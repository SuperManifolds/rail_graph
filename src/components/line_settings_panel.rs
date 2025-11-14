use leptos::{component, view, IntoView, ReadSignal, WriteSignal, SignalGet, SignalSet, event_target_checked, event_target_value};

#[component]
#[must_use]
pub fn LineSettingsPanel(
    /// Whether line mode is active (panel only shows when true)
    show_lines: ReadSignal<bool>,
    /// Whether to hide unscheduled stations and tracks in line mode
    hide_unscheduled: ReadSignal<bool>,
    /// Setter for hide_unscheduled
    set_hide_unscheduled: WriteSignal<bool>,
    /// Line gap width setting
    line_gap_width: ReadSignal<f64>,
    /// Setter for line gap width
    set_line_gap_width: WriteSignal<f64>,
) -> impl IntoView {
    view! {
        <div
            class="line-settings-panel"
            class:hidden=move || !show_lines.get()
        >
            <div class="form-group">
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

            <div class="form-group">
                <label>"Line Spacing"</label>
                <div class="thickness-control">
                    <input
                        type="range"
                        min="0.0"
                        max="10.0"
                        step="0.5"
                        value=move || line_gap_width.get()
                        on:input=move |ev| {
                            let value = event_target_value(&ev).parse::<f64>().unwrap_or(5.0);
                            set_line_gap_width.set(value);
                        }
                    />
                    <span class="thickness-value">
                        {move || format!("{:.1}", line_gap_width.get())}
                    </span>
                </div>
            </div>
        </div>
    }
}
