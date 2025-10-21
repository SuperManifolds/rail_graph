use crate::components::duration_input::DurationInput;
use crate::models::{Line, RouteDirection};
use leptos::{component, view, ReadSignal, IntoView, Signal, SignalGetUntracked};
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
                if index > 0 && index - 1 < updated_line.forward_route.len() {
                    updated_line.forward_route[index - 1].wait_time = new_wait_time;
                }
            }
            RouteDirection::Return => {
                if index > 0 && index - 1 < updated_line.return_route.len() {
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
    wait_duration: Duration,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    if index > 0 {
        view! {
            <DurationInput
                duration=Signal::derive(move || wait_duration)
                on_change={
                    let on_save = on_save.clone();
                    move |new_wait_time| {
                        update_wait_time(edited_line, route_direction, index, new_wait_time, &on_save);
                    }
                }
            />
        }.into_view()
    } else {
        view! { <span class="travel-time">"-"</span> }.into_view()
    }
}
