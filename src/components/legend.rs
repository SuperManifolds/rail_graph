use leptos::{component, view, Signal, IntoView, create_signal, SignalUpdate, Show, SignalGet, event_target_checked};

#[component]
pub fn Legend(
    show_conflicts: Signal<bool>,
    set_show_conflicts: impl Fn(bool) + 'static + Copy,
    show_line_blocks: Signal<bool>,
    set_show_line_blocks: impl Fn(bool) + 'static + Copy,
    spacing_mode: Signal<crate::models::SpacingMode>,
    set_spacing_mode: impl Fn(crate::models::SpacingMode) + 'static + Copy,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);

    let toggle_popover = move |_| {
        set_is_open.update(|open| *open = !*open);
    };

    view! {
        <div class="legend-container">
            <button class="legend-button" on:click=toggle_popover title="Display Options">
                <i class="fa-solid fa-eye"></i>
            </button>

            <Show when=move || is_open.get()>
                <div class="legend-popover">
                    <div class="legend-header">
                        <h3>"Display Options"</h3>
                        <button class="close-button" on:click=toggle_popover>"×"</button>
                    </div>

                    <div class="legend-items">
                        <div class="legend-item">
                            <label class="legend-label">
                                <input
                                    type="checkbox"
                                    checked=move || show_conflicts.get()
                                    on:change=move |ev| {
                                        set_show_conflicts(event_target_checked(&ev));
                                    }
                                />
                                <span class="legend-icon conflict-icon">"⚠"</span>
                                <span>"Conflict Markers"</span>
                            </label>
                            <p class="legend-description">"Track conflicts between trains (yellow triangles)"</p>
                        </div>

                        <div class="legend-item">
                            <label class="legend-label">
                                <input
                                    type="checkbox"
                                    checked=move || show_line_blocks.get()
                                    on:change=move |ev| {
                                        set_show_line_blocks(event_target_checked(&ev));
                                    }
                                />
                                <span class="legend-icon">"▭"</span>
                                <span>"Block Occupancy on Hover"</span>
                            </label>
                            <p class="legend-description">"Show reservation block when hovering over train lines"</p>
                        </div>

                        <div class="legend-item">
                            <label class="legend-label">
                                <input
                                    type="checkbox"
                                    checked=move || matches!(spacing_mode.get(), crate::models::SpacingMode::DistanceBased)
                                    on:change=move |ev| {
                                        let is_checked = event_target_checked(&ev);
                                        set_spacing_mode(if is_checked {
                                            crate::models::SpacingMode::DistanceBased
                                        } else {
                                            crate::models::SpacingMode::Equal
                                        });
                                    }
                                />
                                <span class="legend-icon">"📏"</span>
                                <span>"Distance-based Spacing"</span>
                            </label>
                            <p class="legend-description">"Scale vertical spacing by track distance (if available)"</p>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}