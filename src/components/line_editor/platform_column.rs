use crate::models::{Line, RouteDirection};
use super::{PlatformSelect, PlatformField};
use leptos::{component, view, ReadSignal, IntoView};
use std::rc::Rc;

#[component]
pub fn PlatformColumn(
    platforms: Vec<crate::models::Platform>,
    current_platform_origin: Option<usize>,
    current_platform_dest: Option<usize>,
    index: usize,
    is_first: bool,
    is_last: bool,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    if platforms.is_empty() {
        return view! { <span class="platform-placeholder">"-"</span> }.into_view();
    }

    if is_first {
        if let Some(current_platform) = current_platform_origin {
            return view! {
                <PlatformSelect
                    platforms=platforms
                    current_platform=current_platform
                    index=index
                    field=PlatformField::Origin
                    route_direction=route_direction
                    edited_line=edited_line
                    on_save=on_save
                />
            }.into_view();
        }
    } else if is_last {
        if let Some(current_platform) = current_platform_dest {
            return view! {
                <PlatformSelect
                    platforms=platforms
                    current_platform=current_platform
                    index=index - 1
                    field=PlatformField::Destination
                    route_direction=route_direction
                    edited_line=edited_line
                    on_save=on_save
                />
            }.into_view();
        }
    } else if !is_first && !is_last {
        if let Some(current_platform) = current_platform_dest {
            return view! {
                <PlatformSelect
                    platforms=platforms
                    current_platform=current_platform
                    index=index
                    field=PlatformField::Both
                    route_direction=route_direction
                    edited_line=edited_line
                    on_save=on_save
                />
            }.into_view();
        }
    }

    view! { <span class="platform-placeholder">"-"</span> }.into_view()
}
