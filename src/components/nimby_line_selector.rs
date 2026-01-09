//! NIMBY Rails line selector component

use leptos::{component, view, IntoView, Signal, SignalGet, SignalSet, SignalUpdate, create_signal, For, Callback, Callable, Show, ReadSignal};
use crate::import::nimby::{NimbyImportData, NimbyImportConfig, NimbyLineSummary};
use crate::models::TrackHandedness;

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn NimbyLineSelector(
    data: Signal<NimbyImportData>,
    handedness: Signal<TrackHandedness>,
    station_spacing: Signal<f64>,
    on_cancel: Callback<()>,
    on_import: Callback<NimbyImportConfig>,
    #[prop(optional)] import_error: Option<ReadSignal<Option<String>>>,
) -> impl IntoView {
    let summaries = move || data.get().get_line_summaries();

    // Track selected line IDs (empty by default)
    let (selected_ids, set_selected_ids) = create_signal(Vec::<String>::new());

    // Track import mode (true = create infrastructure, false = pathfinding)
    let (create_infrastructure, set_create_infrastructure) = create_signal(true);

    let toggle_line = move |line_id: String| {
        set_selected_ids.update(|ids| {
            if ids.contains(&line_id) {
                ids.retain(|id| id != &line_id);
            } else {
                ids.push(line_id);
            }
        });
    };

    let select_all = move |_| {
        set_selected_ids.set(summaries().iter().map(|s| s.id.clone()).collect());
    };

    let select_none = move |_| {
        set_selected_ids.set(Vec::new());
    };

    // Track update existing mode (only relevant in schedules mode)
    let (update_existing, set_update_existing) = create_signal(false);

    let handle_import = move |_| {
        let config = NimbyImportConfig {
            create_infrastructure: create_infrastructure.get(),
            selected_line_ids: selected_ids.get(),
            handedness: handedness.get(),
            station_spacing: station_spacing.get(),
            update_existing: update_existing.get(),
        };
        on_import.call(config);
    };

    view! {
        <div class="nimby-line-selector">
            <section class="import-summary">
                <div class="summary-item">
                    <span class="summary-label">"Company"</span>
                    <span class="summary-value">{move || data.get().company_name}</span>
                </div>
                <div class="summary-item">
                    <span class="summary-label">"Stations"</span>
                    <span class="summary-value">{move || data.get().stations.len()}</span>
                </div>
                <div class="summary-item">
                    <span class="summary-label">"Lines"</span>
                    <span class="summary-value">{move || data.get().lines.len()}</span>
                </div>
            </section>

            <section class="mode-selection">
                <h3>"Import Mode"</h3>
                <div class="mode-options">
                    <label class="mode-option">
                        <input
                            type="radio"
                            name="import_mode"
                            checked=move || create_infrastructure.get()
                            on:change=move |_| set_create_infrastructure.set(true)
                        />
                        <div class="mode-content">
                            <span class="mode-title">"Import Infrastructure"</span>
                            <span class="mode-description">
                                "Creates stations and tracks. Analyzes all lines for optimal layout."
                            </span>
                        </div>
                    </label>
                    <label class="mode-option">
                        <input
                            type="radio"
                            name="import_mode"
                            checked=move || !create_infrastructure.get()
                            on:change=move |_| set_create_infrastructure.set(false)
                        />
                        <div class="mode-content">
                            <span class="mode-title">"Import Schedules"</span>
                            <span class="mode-description">
                                "Creates lines with routes and timing. Uses existing infrastructure."
                            </span>
                        </div>
                    </label>
                </div>
                <Show when=move || !create_infrastructure.get()>
                    <label class="update-existing-option">
                        <input
                            type="checkbox"
                            checked=move || update_existing.get()
                            on:change=move |_| set_update_existing.set(!update_existing.get())
                        />
                        <div class="mode-content">
                            <span class="mode-title">"Update existing lines"</span>
                            <span class="mode-description">
                                "Match by line code and update routes. Preserves wait times."
                            </span>
                        </div>
                    </label>
                </Show>
            </section>

            <section class="line-selection">
                    <header class="selection-header">
                        <h3>"Select Lines to Import"</h3>
                        <div class="bulk-actions">
                            <button
                                type="button"
                                class="preset-button"
                                on:click=select_all
                            >"All"</button>
                            <button
                                type="button"
                                class="preset-button"
                                on:click=select_none
                            >"None"</button>
                        </div>
                    </header>

                    <div class="line-list">
                        <For
                            each=summaries
                            key=|s| s.id.clone()
                            children=move |summary: NimbyLineSummary| {
                                let line_id = summary.id.clone();
                                let line_id_toggle = line_id.clone();
                                let is_selected = move || selected_ids.get().contains(&line_id);

                                view! {
                                    <label class="line-item">
                                        <input
                                            type="checkbox"
                                            checked=is_selected
                                            on:change=move |_| toggle_line(line_id_toggle.clone())
                                        />
                                        <span
                                            class="line-code"
                                            style=format!("background-color: {}; color: {}", summary.color, summary.text_color)
                                        >{summary.code.clone()}</span>
                                        <span class="line-name">{summary.name.clone()}</span>
                                        <span class="stop-count">{format!("({} stops)", summary.stop_count)}</span>
                                    </label>
                                }
                            }
                        />
                    </div>
            </section>

            <Show when=move || {
                import_error.is_some_and(|e| e.get().is_some())
            }>
                <div class="mapper-error">
                    <i class="fa-solid fa-triangle-exclamation"></i>
                    <span>{move || import_error.map_or(String::new(), |e| e.get().unwrap_or_default())}</span>
                </div>
            </Show>

            <footer class="mapper-actions">
                <button
                    type="button"
                    on:click=move |_| on_cancel.call(())
                >"Cancel"</button>
                <button
                    type="button"
                    class="primary"
                    disabled=move || selected_ids.get().is_empty()
                    on:click=handle_import
                >
                    {move || {
                        let count = selected_ids.get().len();
                        if create_infrastructure.get() {
                            format!("Import Infrastructure ({count} lines)")
                        } else {
                            format!("Import {count} Lines")
                        }
                    }}
                </button>
            </footer>
        </div>
    }
}
