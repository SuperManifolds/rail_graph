use leptos::*;
use crate::components::graph_canvas::{LEFT_MARGIN, TOP_MARGIN, RIGHT_PADDING, BOTTOM_PADDING};
use crate::models::{Conflict, Station};

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

#[allow(clippy::too_many_arguments)]
pub fn check_conflict_hover(
    mouse_x: f64,
    mouse_y: f64,
    conflicts: &[Conflict],
    stations: &[Station],
    canvas_width: f64,
    canvas_height: f64,
    zoom_level: f64,
    pan_offset_x: f64,
    pan_offset_y: f64,
) -> Option<(Conflict, f64, f64)> {
    let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;
    let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;

    // Check if mouse is within the graph area first
    if mouse_x < LEFT_MARGIN || mouse_x > LEFT_MARGIN + graph_width ||
       mouse_y < TOP_MARGIN || mouse_y > TOP_MARGIN + graph_height {
        return None;
    }

    for conflict in conflicts {
        // Calculate conflict position in screen coordinates
        // The canvas uses: translate(LEFT_MARGIN, TOP_MARGIN) + translate(pan) + scale(zoom)
        let time_fraction = crate::time::time_to_fraction(conflict.time);
        let total_hours = 48.0;
        let hour_width = graph_width / total_hours;

        // Position in zoomed coordinate system (before translation)
        let x_in_zoomed = time_fraction * hour_width;

        let station_height = graph_height / stations.len() as f64;
        let y_in_zoomed = (conflict.station1_idx as f64 * station_height) +
            (station_height / 2.0) +
            (conflict.position * station_height * (conflict.station2_idx - conflict.station1_idx) as f64);

        // Transform to screen coordinates
        let screen_x = LEFT_MARGIN + (x_in_zoomed * zoom_level) + pan_offset_x;
        let screen_y = TOP_MARGIN + (y_in_zoomed * zoom_level) + pan_offset_y;

        // Check if mouse is within conflict marker bounds
        let size = 15.0;
        if mouse_x >= screen_x - size && mouse_x <= screen_x + size &&
           mouse_y >= screen_y - size && mouse_y <= screen_y + size {
            return Some((conflict.clone(), mouse_x, mouse_y));
        }
    }

    None
}