use leptos::*;
use leptos::leptos_dom::helpers::window_event_listener;
use wasm_bindgen::JsCast;
use crate::models::Conflict;
use crate::time::time_to_fraction;

#[component]
fn ErrorListPopover(
    conflicts: Signal<Vec<Conflict>>,
    on_conflict_click: impl Fn(f64, f64) + 'static + Copy,
) -> impl IntoView {
    view! {
        <div class="error-list-popover">
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

                                    let time_fraction = time_to_fraction(conflict.time);
                                    let station_position = conflict.station1_idx as f64 + conflict.position;

                                    view! {
                                        <div
                                            class="error-item clickable"
                                            on:click=move |_| {
                                                on_conflict_click(time_fraction, station_position);
                                            }
                                        >
                                            <div class="error-item-header">
                                                <i class="fa-solid fa-triangle-exclamation"></i>
                                                <span class="error-type">{conflict_type}</span>
                                            </div>
                                            <div class="error-item-details">
                                                <div class="error-detail">
                                                    <span class="value">{format!("{} × {}", conflict.journey1_id, conflict.journey2_id)}</span>
                                                </div>
                                                <div class="error-detail">
                                                    <span class="value">{conflict.time.format("%H:%M:%S").to_string()}</span>
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
    }
}

#[component]
pub fn ErrorList(
    conflicts: Signal<Vec<Conflict>>,
    on_conflict_click: impl Fn(f64, f64) + 'static + Copy,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);

    let toggle_popover = move |_| {
        set_is_open.update(|open| *open = !*open);
    };

    let conflict_count = move || conflicts.get().len();
    let has_errors = move || conflict_count() > 0;

    // Close when clicking outside
    let container_ref = create_node_ref::<leptos::html::Div>();

    window_event_listener(leptos::ev::click, move |ev| {
        if !is_open.get() {
            return;
        }
        let Some(container) = container_ref.get() else {
            return;
        };
        let target = ev.target();
        let Some(target_element) = target.and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else {
            return;
        };
        if !container.contains(Some(&target_element)) {
            set_is_open.set(false);
        }
    });

    view! {
        <div class="error-list-container" node_ref=container_ref>
            {move || {
                if has_errors() {
                    view! {
                        <button
                            class="error-list-button has-errors"
                            on:click=toggle_popover
                        >
                            <i class="fa-solid fa-triangle-exclamation"></i>
                            <span class="error-count">{conflict_count()}</span>
                            <span class="error-label">" Conflicts"</span>
                        </button>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            {move || {
                if is_open.get() {
                    view! {
                        <ErrorListPopover
                            conflicts=conflicts
                            on_conflict_click=on_conflict_click
                        />
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}
