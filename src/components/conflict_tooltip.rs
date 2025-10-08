use leptos::*;
use crate::conflict::{Conflict, ConflictType};

#[component]
pub fn ConflictTooltip(
    hovered_conflict: ReadSignal<Option<(Conflict, f64, f64)>>,
) -> impl IntoView {
    view! {
        {move || {
            if let Some((conflict, tooltip_x, tooltip_y)) = hovered_conflict.get() {
                // Determine conflict type text and train order
                let (conflict_text, first_train, second_train) = match conflict.conflict_type {
                    ConflictType::Overtaking => {
                        // For overtaking, swap the order
                        ("overtakes", &conflict.journey2_id, &conflict.journey1_id)
                    }
                    ConflictType::HeadOn => {
                        ("conflicts with", &conflict.journey1_id, &conflict.journey2_id)
                    }
                    ConflictType::BlockViolation => {
                        ("block violation with", &conflict.journey1_id, &conflict.journey2_id)
                    }
                };

                let tooltip_text = format!(
                    "{} {} {} at {}",
                    first_train,
                    conflict_text,
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