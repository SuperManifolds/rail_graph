use crate::components::duration_input::DurationInput;
use crate::models::{Line, RouteDirection};
use leptos::{component, view, ReadSignal, IntoView, Signal, SignalGetUntracked, SignalWith};
use chrono::Duration;
use std::rc::Rc;

fn update_wait_time(
    edited_line: ReadSignal<Option<Line>>,
    route_direction: RouteDirection,
    index: usize,
    new_wait_time: Duration,
    on_save: &Rc<dyn Fn(Line)>,
) {
    if let Some(mut updated_line) = edited_line.get_untracked() {
        match route_direction {
            RouteDirection::Forward => {
                if index == 0 {
                    updated_line.first_stop_wait_time = new_wait_time;
                } else if index - 1 < updated_line.forward_route.len() {
                    updated_line.forward_route[index - 1].wait_time = new_wait_time;
                }
                // Sync wait time to return route if sync is enabled
                updated_line.apply_route_sync_if_enabled();
            }
            RouteDirection::Return => {
                if index == 0 {
                    updated_line.return_first_stop_wait_time = new_wait_time;
                } else if index - 1 < updated_line.return_route.len() {
                    updated_line.return_route[index - 1].wait_time = new_wait_time;
                }
            }
        }
        on_save(updated_line);
    }
}

#[component]
pub fn WaitTimeColumn(
    index: usize,
    #[prop(optional)] wait_duration: Option<Duration>,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
    is_junction: bool,
    is_first: bool,
) -> impl IntoView {
    if is_junction {
        // Junctions never have wait time - show placeholder
        return view! { <span class="track-placeholder">"-"</span> }.into_view();
    }

    // Compute wait_duration reactively from edited_line
    #[allow(clippy::excessive_nesting)]
    let wait_duration_signal = Signal::derive(move || {
        edited_line.with(|line| {
            line.as_ref().map_or(wait_duration.unwrap_or(Duration::zero()), |l| {
                if is_first {
                    match route_direction {
                        RouteDirection::Forward => l.first_stop_wait_time,
                        RouteDirection::Return => l.return_first_stop_wait_time,
                    }
                } else {
                    let route = match route_direction {
                        RouteDirection::Forward => &l.forward_route,
                        RouteDirection::Return => &l.return_route,
                    };
                    if index > 0 && index - 1 < route.len() {
                        route[index - 1].wait_time
                    } else {
                        Duration::zero()
                    }
                }
            })
        })
    });

    view! {
        <DurationInput
            duration=wait_duration_signal
            on_change={
                let on_save = on_save.clone();
                move |new_wait_time| {
                    update_wait_time(edited_line, route_direction, index, new_wait_time, &on_save);
                }
            }
        />
    }.into_view()
}
