use crate::models::DaysOfWeek;
use leptos::{component, view, Signal, IntoView, SignalGet, SignalGetUntracked};

#[component]
#[must_use]
pub fn DaysOfWeekSelector<F>(
    days_of_week: Signal<DaysOfWeek>,
    set_days_of_week: F,
    #[prop(optional)] label: Option<&'static str>,
) -> impl IntoView
where
    F: Fn(DaysOfWeek) + 'static + Clone,
{
    let label_text = label.unwrap_or("Operating days:");

    let set_all_days = {
        let set_days_of_week = set_days_of_week.clone();
        move |_| set_days_of_week(DaysOfWeek::ALL_DAYS)
    };

    view! {
        <div class="days-of-week-selector">
            <label>{label_text}</label>
            <div class="preset-buttons">
                <button
                    type="button"
                    class="preset-button"
                    on:click=set_all_days
                    title="Select all days"
                >
                    "All days"
                </button>
            </div>
            <div class="day-checkboxes">
                {[
                    (DaysOfWeek::MONDAY, "Mon", "Monday"),
                    (DaysOfWeek::TUESDAY, "Tue", "Tuesday"),
                    (DaysOfWeek::WEDNESDAY, "Wed", "Wednesday"),
                    (DaysOfWeek::THURSDAY, "Thu", "Thursday"),
                    (DaysOfWeek::FRIDAY, "Fri", "Friday"),
                    (DaysOfWeek::SATURDAY, "Sat", "Saturday"),
                    (DaysOfWeek::SUNDAY, "Sun", "Sunday"),
                ].iter().map(|(day, short, full)| {
                    let day_value = *day;
                    let set_days_of_week = set_days_of_week.clone();
                    view! {
                        <label class="day-checkbox">
                            <input
                                type="checkbox"
                                checked=move || days_of_week.get().contains(day_value)
                                on:change=move |_| {
                                    let current = days_of_week.get_untracked();
                                    let mut new_days = current;
                                    new_days.toggle(day_value);
                                    set_days_of_week(new_days);
                                }
                            />
                            <span class="day-short">{*short}</span>
                            <span class="day-full">{*full}</span>
                        </label>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
