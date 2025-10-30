use super::stop_row::calculate_cumulative_seconds;
use crate::components::{duration_input::OptionalDurationInput, time_input::TimeInput};
use crate::models::{Line, RouteDirection};
use crate::constants::BASE_MIDNIGHT;
use crate::time::format_duration_hms;
use leptos::{component, view, ReadSignal, IntoView, SignalGetUntracked, SignalGet, Show};
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
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    use leptos::{Signal, SignalWith};

    // Compute all values reactively from edited_line
    let segment_duration = Signal::derive(move || {
        edited_line.with(|line| {
            line.as_ref().and_then(|l| {
                let route = match route_direction {
                    RouteDirection::Forward => &l.forward_route,
                    RouteDirection::Return => &l.return_route,
                };

                let display_durations: Vec<Option<Duration>> = if matches!(route_direction, RouteDirection::Return) && l.sync_routes {
                    l.get_return_display_durations()
                } else {
                    route.iter().map(|s| s.duration).collect()
                };

                display_durations.get(index).copied().flatten()
            })
        })
    });

    let cumulative_seconds = Signal::derive(move || {
        edited_line.with(|line| {
            line.as_ref().map_or(0, |l| {
                let route = match route_direction {
                    RouteDirection::Forward => &l.forward_route,
                    RouteDirection::Return => &l.return_route,
                };

                let display_durations: Vec<Option<Duration>> = if matches!(route_direction, RouteDirection::Return) && l.sync_routes {
                    l.get_return_display_durations()
                } else {
                    route.iter().map(|s| s.duration).collect()
                };

                calculate_cumulative_seconds(&display_durations, route, index)
            })
        })
    });

    let sync_routes = Signal::derive(move || {
        edited_line.with(|line| {
            line.as_ref().is_some_and(|l| l.sync_routes)
        })
    });

    // Disable return route time inputs when sync is enabled (times calculated from forward route)
    let is_disabled = Signal::derive(move || matches!(route_direction, RouteDirection::Return) && sync_routes.get());

    match time_mode {
        TimeDisplayMode::Difference => {
            view! {
                <div class="time-input-with-preview">
                    <Show
                        when=move || is_disabled.get()
                        fallback=move || view! {
                            <OptionalDurationInput
                                duration=segment_duration
                                on_change={
                                    let on_save = on_save.clone();
                                    move |new_duration| {
                                        update_segment_duration(edited_line, route_direction, index, new_duration, &on_save);
                                    }
                                }
                            />
                        }
                    >
                        {move || {
                            let display_text = segment_duration.get()
                                .map_or_else(|| "—".to_string(), format_duration_hms);
                            view! {
                                <span class="travel-time disabled" title="Time calculated from forward route">{display_text}</span>
                            }
                        }}
                    </Show>
                    <span class="cumulative-preview">{move || format_cumulative_time(cumulative_seconds.get())}</span>
                </div>
            }.into_view()
        }
        TimeDisplayMode::Absolute => {
            if index > 0 {
                view! {
                    <Show
                        when=move || is_disabled.get()
                        fallback=move || {
                            let cumulative_time_signal = Signal::derive(move || BASE_MIDNIGHT + Duration::seconds(cumulative_seconds.get()));
                            view! {
                                <TimeInput
                                    label=""
                                    value=cumulative_time_signal
                                    default_time="00:00:00"
                                    on_change={
                                        let on_save = on_save.clone();
                                        Box::new(move |new_time| {
                                            let new_cumulative_seconds = (new_time - BASE_MIDNIGHT).num_seconds();
                                            update_absolute_time(edited_line, route_direction, index, new_cumulative_seconds, &on_save);
                                        })
                                    }
                                />
                            }
                        }
                    >
                        {move || {
                            let cum_secs = cumulative_seconds.get();
                            let display_text = {
                                let hours = cum_secs / 3600;
                                let minutes = (cum_secs % 3600) / 60;
                                let seconds = cum_secs % 60;
                                format!("{hours:02}:{minutes:02}:{seconds:02}")
                            };
                            view! {
                                <span class="travel-time disabled" title="Time calculated from forward route">{display_text}</span>
                            }
                        }}
                    </Show>
                }.into_view()
            } else {
                view! { <span class="travel-time">"00:00:00"</span> }.into_view()
            }
        }
    }
}
