use leptos::*;
use crate::models::Conflict;

#[component]
pub fn ErrorList(
    conflicts: Signal<Vec<Conflict>>,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);

    let toggle_popover = move |_| {
        set_is_open.update(|open| *open = !*open);
    };

    let conflict_count = move || conflicts.get().len();
    let has_errors = move || conflict_count() > 0;

    view! {
        <div class="error-list-container">
            <button
                class="error-list-button"
                class=("has-errors", has_errors)
                on:click=toggle_popover
            >
                <i class="fa-solid fa-triangle-exclamation"></i>
                <span class="error-count">{conflict_count}</span>
                <span class="error-label">" Conflicts"</span>
            </button>

            {move || {
                if is_open.get() {
                    view! {
                        <div class="error-list-popover">
                            <div class="error-list-header">
                                <h3>"Conflicts"</h3>
                                <button class="close-button" on:click=toggle_popover>"×"</button>
                            </div>

                            <div class="error-list-content">
                                {move || {
                                    let current_conflicts = conflicts.get();
                                    if current_conflicts.is_empty() {
                                        view! {
                                            <p class="no-errors">"No conflicts detected"</p>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="error-items">
                                                {current_conflicts.into_iter().map(|conflict| {
                                                    let conflict_type = if conflict.is_overtaking {
                                                        "Overtaking"
                                                    } else {
                                                        "Crossing"
                                                    };

                                                    view! {
                                                        <div class="error-item">
                                                            <div class="error-item-header">
                                                                <i class="fa-solid fa-triangle-exclamation"></i>
                                                                <span class="error-type">{conflict_type}</span>
                                                            </div>
                                                            <div class="error-item-details">
                                                                <div class="error-detail">
                                                                    <span class="label">"Lines: "</span>
                                                                    <span class="value">{format!("{} × {}", conflict.journey1_id, conflict.journey2_id)}</span>
                                                                </div>
                                                                <div class="error-detail">
                                                                    <span class="label">"Time: "</span>
                                                                    <span class="value">{conflict.time.format("%H:%M:%S").to_string()}</span>
                                                                </div>
                                                                <div class="error-detail">
                                                                    <span class="label">"Segment: "</span>
                                                                    <span class="value">{format!("{} → {}", conflict.station1_idx, conflict.station2_idx)}</span>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_view()
                                    }
                                }}
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}
