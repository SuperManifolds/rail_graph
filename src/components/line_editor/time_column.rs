use crate::components::{duration_input::OptionalDurationInput, time_input::TimeInput};
use crate::models::{Line, RouteDirection};
use crate::constants::BASE_MIDNIGHT;
use leptos::{component, view, ReadSignal, IntoView, Signal, SignalGetUntracked};
use chrono::Duration;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
pub enum TimeDisplayMode {
    Difference,  // Time between consecutive stops
    Absolute,    // Cumulative time from start
}

fn format_cumulative_time(cumulative_seconds: i64) -> String {
    let hours = cumulative_seconds / 3600;
    let minutes = (cumulative_seconds % 3600) / 60;
    let seconds = cumulative_seconds % 60;
    format!("(Σ {hours:02}:{minutes:02}:{seconds:02})")
}

fn update_segment_duration(
    edited_line: ReadSignal<Option<Line>>,
    route_direction: RouteDirection,
    index: usize,
    new_duration: Option<Duration>,
    on_save: &Rc<dyn Fn(Line)>,
) {
    if let Some(mut updated_line) = edited_line.get_untracked() {
        match route_direction {
            RouteDirection::Forward => {
                if index < updated_line.forward_route.len() {
                    updated_line.forward_route[index].duration = new_duration;
                }
            }
            RouteDirection::Return => {
                if index < updated_line.return_route.len() {
                    updated_line.return_route[index].duration = new_duration;
                }
            }
        }

        if matches!(route_direction, RouteDirection::Forward) {
            updated_line.apply_route_sync_if_enabled();
        }

        on_save(updated_line);
    }
}

fn update_absolute_time(
    edited_line: ReadSignal<Option<Line>>,
    route_direction: RouteDirection,
    index: usize,
    new_cumulative_seconds: i64,
    on_save: &Rc<dyn Fn(Line)>,
) {
    if let Some(mut updated_line) = edited_line.get_untracked() {
        match route_direction {
            RouteDirection::Forward => {
                let prev_cumulative_seconds: i64 = updated_line.forward_route.iter()
                    .take(index - 1)
                    .map(|seg| (seg.duration.unwrap_or(Duration::zero()) + seg.wait_time).num_seconds())
                    .sum();
                let prev_wait_seconds = updated_line.forward_route[index - 1].wait_time.num_seconds();
                let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                if segment_duration_seconds >= 0 {
                    updated_line.forward_route[index - 1].duration = Some(Duration::seconds(segment_duration_seconds));
                    updated_line.apply_route_sync_if_enabled();
                    on_save(updated_line);
                }
            }
            RouteDirection::Return => {
                let prev_cumulative_seconds: i64 = updated_line.return_route.iter()
                    .take(index - 1)
                    .map(|seg| (seg.duration.unwrap_or(Duration::zero()) + seg.wait_time).num_seconds())
                    .sum();
                let prev_wait_seconds = updated_line.return_route[index - 1].wait_time.num_seconds();
                let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                if segment_duration_seconds >= 0 {
                    updated_line.return_route[index - 1].duration = Some(Duration::seconds(segment_duration_seconds));
                    on_save(updated_line);
                }
            }
        }
    }
}

#[component]
pub fn TimeColumn(
    time_mode: TimeDisplayMode,
    index: usize,
    segment_duration: Option<Duration>,
    cumulative_seconds: i64,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    match time_mode {
        TimeDisplayMode::Difference => {
            let preview_text = format_cumulative_time(cumulative_seconds);

            view! {
                <div class="time-input-with-preview">
                    <OptionalDurationInput
                        duration=Signal::derive(move || segment_duration)
                        on_change={
                            let on_save = on_save.clone();
                            move |new_duration| {
                                update_segment_duration(edited_line, route_direction, index, new_duration, &on_save);
                            }
                        }
                    />
                    <span class="cumulative-preview">{preview_text}</span>
                </div>
            }.into_view()
        }
        TimeDisplayMode::Absolute => {
            if index > 0 {
                let cumulative_time = BASE_MIDNIGHT + Duration::seconds(cumulative_seconds);
                view! {
                    <TimeInput
                        label=""
                        value=Signal::derive(move || cumulative_time)
                        default_time="00:00:00"
                        on_change={
                            let on_save = on_save.clone();
                            Box::new(move |new_time| {
                                let new_cumulative_seconds = (new_time - BASE_MIDNIGHT).num_seconds();
                                update_absolute_time(edited_line, route_direction, index, new_cumulative_seconds, &on_save);
                            })
                        }
                    />
                }.into_view()
            } else {
                view! { <span class="travel-time">"00:00:00"</span> }.into_view()
            }
        }
    }
}
