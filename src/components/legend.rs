use leptos::{component, view, Signal, IntoView, create_signal, SignalUpdate, Show, SignalGet, event_target_checked};

#[component]
pub fn Legend(
    show_station_crossings: Signal<bool>,
    set_show_station_crossings: impl Fn(bool) + 'static + Copy,
    show_conflicts: Signal<bool>,
    set_show_conflicts: impl Fn(bool) + 'static + Copy,
    show_line_blocks: Signal<bool>,
    set_show_line_blocks: impl Fn(bool) + 'static + Copy,
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
                                    checked=move || show_station_crossings.get()
                                    on:change=move |ev| {
                                        set_show_station_crossings(event_target_checked(&ev));
                                    }
                                />
                                <span class="legend-icon station-crossing-icon"></span>
                                <span>"Station Crossings"</span>
                            </label>
                            <p class="legend-description">"Successful passes at stations (green circles)"</p>
                        </div>

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
                    </div>
                </div>
            </Show>
        </div>
    }
}