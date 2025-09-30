use leptos::*;
use crate::components::time_graph::Conflict;

#[component]
pub fn ConflictTooltip(
    hovered_conflict: ReadSignal<Option<(Conflict, f64, f64)>>,
) -> impl IntoView {
    view! {
        {move || {
            if let Some((conflict, tooltip_x, tooltip_y)) = hovered_conflict.get() {
                // Use the stored flag to determine conflict type
                let conflict_type = if conflict.is_overtaking {
                    "overtakes"
                } else {
                    "conflicts with"
                };

                let (first_train, second_train) = if conflict_type == "overtakes" {
                    // For overtaking, swap the order
                    (&conflict.journey2_id, &conflict.journey1_id)
                } else {
                    // For crossing conflicts, keep original order
                    (&conflict.journey1_id, &conflict.journey2_id)
                };

                let tooltip_text = format!(
                    "{} {} {} at {}",
                    first_train,
                    conflict_type,
                    second_train,
                    conflict.time.format("%H:%M")
                );

                view! {
                    <div
                        class="conflict-tooltip"
                        style=format!("left: {}px; top: {}px;", tooltip_x + 10.0, tooltip_y - 30.0)
                    >
                        {tooltip_text}
                    </div>
                }.into_view()
            } else {
                view! { <div class="conflict-tooltip-hidden"></div> }.into_view()
            }
        }}
    }
}