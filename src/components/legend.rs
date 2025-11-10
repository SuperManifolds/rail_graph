use leptos::{component, view, Signal, IntoView, create_signal, SignalGet, SignalSet, event_target_checked, Portal, create_node_ref, html, SignalGetUntracked};

const POPOVER_ESTIMATED_WIDTH: f64 = 300.0;
const POPOVER_ESTIMATED_HEIGHT: f64 = 250.0;
const POPOVER_SPACING: f64 = 8.0;

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
    let (popover_position, set_popover_position) = create_signal((0.0, 0.0));
    let button_ref = create_node_ref::<html::Button>();

    let toggle_popover = move |_| {
        let new_state = !is_open.get_untracked();

        // Calculate position when opening
        if new_state {
            if let Some(button_el) = button_ref.get() {
                let rect = button_el.get_bounding_client_rect();
                let button_top = rect.top();
                let button_bottom = rect.bottom();
                let button_left = rect.left();
                let button_right = rect.right();

                // Get viewport dimensions
                let viewport_width = web_sys::window()
                    .and_then(|w| w.inner_width().ok())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1920.0);
                let viewport_height = web_sys::window()
                    .and_then(|w| w.inner_height().ok())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1080.0);

                // Check vertical positioning: prefer below, but use above if not enough space
                let y = if button_bottom + POPOVER_SPACING + POPOVER_ESTIMATED_HEIGHT <= viewport_height {
                    // Enough space below
                    button_bottom + POPOVER_SPACING
                } else if button_top - POPOVER_SPACING - POPOVER_ESTIMATED_HEIGHT >= 0.0 {
                    // Not enough space below, but enough above
                    button_top - POPOVER_SPACING - POPOVER_ESTIMATED_HEIGHT
                } else {
                    // Not enough space either direction, position at top with some margin
                    10.0
                };

                // Check horizontal positioning: prefer left-aligned with button, but right-align if it would overflow
                let x = if button_left + POPOVER_ESTIMATED_WIDTH <= viewport_width {
                    // Enough space when left-aligned with button
                    button_left
                } else if button_right - POPOVER_ESTIMATED_WIDTH >= 0.0 {
                    // Not enough space left-aligned, right-align with button
                    button_right - POPOVER_ESTIMATED_WIDTH
                } else {
                    // Not enough space, position from right edge of viewport
                    viewport_width - POPOVER_ESTIMATED_WIDTH - 10.0
                };

                set_popover_position.set((x, y));
            }
        }

        set_is_open.set(new_state);
    };

    view! {
        <div class="legend-container">
            <button class="legend-button" on:click=toggle_popover title="Display Options" node_ref=button_ref>
                <i class="fa-solid fa-eye"></i>
            </button>

            {move || {
                if is_open.get() {
                    view! {
                        <Portal>
                            <div
                                class="legend-popover"
                                style=move || {
                                    let (x, y) = popover_position.get();
                                    format!("position: fixed; left: {x}px; top: {y}px; z-index: 9999;")
                                }
                            >
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
                        </Portal>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}