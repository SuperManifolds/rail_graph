use crate::models::{Platform, Line, RouteDirection};
use leptos::{component, view, ReadSignal, Props, IntoView, event_target_value, SignalGetUntracked};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
pub enum PlatformField {
    Origin,       // Update origin_platform of segment at index
    Destination,  // Update destination_platform of segment at index
    Both,         // Update both destination of segment at index-1 and origin of segment at index
}

#[component]
pub fn PlatformSelect(
    #[prop(into)] platforms: Vec<Platform>,
    current_platform: usize,
    index: usize,
    field: PlatformField,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    view! {
        <select
            class="platform-select"
            on:change=move |ev| {
                if let Ok(platform_idx) = event_target_value(&ev).parse::<usize>() {
                    if let Some(mut updated_line) = edited_line.get_untracked() {
                        match route_direction {
                            RouteDirection::Forward => {
                                match field {
                                    PlatformField::Origin => {
                                        if index < updated_line.forward_route.len() {
                                            updated_line.forward_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                    PlatformField::Destination => {
                                        if index < updated_line.forward_route.len() {
                                            updated_line.forward_route[index].destination_platform = platform_idx;
                                        }
                                    }
                                    PlatformField::Both => {
                                        if index > 0 && index - 1 < updated_line.forward_route.len() {
                                            updated_line.forward_route[index - 1].destination_platform = platform_idx;
                                        }
                                        if index < updated_line.forward_route.len() {
                                            updated_line.forward_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                }
                            }
                            RouteDirection::Return => {
                                match field {
                                    PlatformField::Origin => {
                                        if index < updated_line.return_route.len() {
                                            updated_line.return_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                    PlatformField::Destination => {
                                        if index < updated_line.return_route.len() {
                                            updated_line.return_route[index].destination_platform = platform_idx;
                                        }
                                    }
                                    PlatformField::Both => {
                                        if index > 0 && index - 1 < updated_line.return_route.len() {
                                            updated_line.return_route[index - 1].destination_platform = platform_idx;
                                        }
                                        if index < updated_line.return_route.len() {
                                            updated_line.return_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                }
                            }
                        }
                        on_save(updated_line);
                    }
                }
            }
        >
            {platforms.iter().enumerate().map(|(i, platform)| {
                view! {
                    <option value=i.to_string() selected=i == current_platform>
                        {platform.name.clone()}
                    </option>
                }
            }).collect::<Vec<_>>()}
        </select>
    }
}
