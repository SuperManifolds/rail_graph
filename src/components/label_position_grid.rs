use leptos::{component, view, IntoView, Signal, Callback, SignalGet, Callable};
use crate::components::infrastructure_canvas::station_renderer::LabelPosition;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LabelPositionState {
    /// All selected nodes have the same position
    Single(LabelPosition),
    /// Selected nodes have different positions
    Mixed,
    /// All selected nodes have no override (auto-placement)
    Auto,
}

#[component]
#[must_use]
pub fn LabelPositionGrid(
    /// Whether the grid is visible
    is_open: Signal<bool>,
    /// Callback when user selects a position (None = reset to auto)
    on_select: Callback<Option<LabelPosition>>,
    /// Current state of selected nodes
    current_state: Signal<LabelPositionState>,
) -> impl IntoView {
    let positions: Vec<(Option<LabelPosition>, &'static str)> = vec![
        (Some(LabelPosition::TopLeft), "↖"),
        (Some(LabelPosition::Top), "↑"),
        (Some(LabelPosition::TopRight), "↗"),
        (Some(LabelPosition::Left), "←"),
        (None, "⊙"), // Center = reset to auto
        (Some(LabelPosition::Right), "→"),
        (Some(LabelPosition::BottomLeft), "↙"),
        (Some(LabelPosition::Bottom), "↓"),
        (Some(LabelPosition::BottomRight), "↘"),
    ];

    let is_position_active = move |pos: Option<LabelPosition>| {
        match (current_state.get(), pos) {
            (LabelPositionState::Single(current), Some(p)) => current == p,
            (LabelPositionState::Auto, None) => true,
            _ => false,
        }
    };

    view! {
        <div
            class="label-position-grid"
            class:hidden=move || !is_open.get()
        >
            <div class="label-position-buttons">
                    {positions.into_iter().map(|(pos, arrow)| {
                        let is_active = is_position_active(pos);
                        view! {
                            <button
                                class="position-button"
                                class:active=is_active
                                class:center=pos.is_none()
                                on:click=move |_| {
                                    on_select.call(pos);
                                }
                                title=move || match pos {
                                    Some(LabelPosition::TopLeft) => "Top Left",
                                    Some(LabelPosition::Top) => "Top",
                                    Some(LabelPosition::TopRight) => "Top Right",
                                    Some(LabelPosition::Left) => "Left",
                                    Some(LabelPosition::Right) => "Right",
                                    Some(LabelPosition::BottomLeft) => "Bottom Left",
                                    Some(LabelPosition::Bottom) => "Bottom",
                                    Some(LabelPosition::BottomRight) => "Bottom Right",
                                    None => "Auto (reset to automatic positioning)",
                                }
                            >
                                {arrow}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
