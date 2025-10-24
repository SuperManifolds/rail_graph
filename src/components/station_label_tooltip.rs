use leptos::{component, IntoView, ReadSignal, SignalGet, view};

#[component]
#[must_use]
pub fn StationLabelTooltip(
    hovered_station_label: ReadSignal<Option<(String, f64, f64)>>,
) -> impl IntoView {
    view! {
        {move || {
            if let Some((station_name, tooltip_x, tooltip_y)) = hovered_station_label.get() {
                view! {
                    <div
                        class="station-label-tooltip"
                        style=format!("left: {}px; top: {}px;", tooltip_x + 10.0, tooltip_y - 30.0)
                    >
                        {station_name}
                    </div>
                }.into_view()
            } else {
                view! { <div class="station-label-tooltip-hidden"></div> }.into_view()
            }
        }}
    }
}
