use leptos::*;
use chrono::NaiveTime;

#[component]
pub fn TimeDisplay(current_time: ReadSignal<NaiveTime>) -> impl IntoView {
    view! {
        <div class="current-time">
            "Current Time: "
            <span class="time-display">
                {move || current_time.get().format("%H:%M:%S").to_string()}
            </span>
        </div>
    }
}