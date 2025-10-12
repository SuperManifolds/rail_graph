use leptos::{component, view, ReadSignal, WriteSignal, IntoView, SignalGet, SignalSet};
use chrono::Weekday;

#[component]
#[must_use]
pub fn DaySelector(
    selected_day: ReadSignal<Option<Weekday>>,
    set_selected_day: WriteSignal<Option<Weekday>>,
) -> impl IntoView {
    let days = [
        (Some(Weekday::Mon), "Mon"),
        (Some(Weekday::Tue), "Tue"),
        (Some(Weekday::Wed), "Wed"),
        (Some(Weekday::Thu), "Thu"),
        (Some(Weekday::Fri), "Fri"),
        (Some(Weekday::Sat), "Sat"),
        (Some(Weekday::Sun), "Sun"),
    ];

    view! {
        <div class="day-selector">
            <label>"Filter by day:"</label>
            <div class="day-buttons">
                <button
                    class=move || if selected_day.get().is_none() { "day-button active" } else { "day-button" }
                    on:click=move |_| set_selected_day.set(None)
                    title="Show all days"
                >
                    "All"
                </button>
                {days.iter().map(|(day, label)| {
                    let day_value = *day;
                    view! {
                        <button
                            class=move || {
                                if selected_day.get() == day_value {
                                    "day-button active"
                                } else {
                                    "day-button"
                                }
                            }
                            on:click=move |_| set_selected_day.set(day_value)
                            title=format!("Show only {}", label)
                        >
                            {*label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
