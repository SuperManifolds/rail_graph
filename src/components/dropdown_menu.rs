use leptos::{component, view, IntoView, create_signal, SignalGet, SignalSet, SignalUpdate, create_node_ref};
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use std::rc::Rc;

#[derive(Clone)]
pub struct MenuItem {
    pub label: &'static str,
    pub icon: &'static str,
    pub on_click: Rc<dyn Fn() + 'static>,
}

#[component]
#[must_use]
pub fn DropdownMenu(items: Vec<MenuItem>) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);
    let menu_ref = create_node_ref::<leptos::html::Div>();

    // Close menu when clicking outside
    let close_on_outside_click = move |event: MouseEvent| {
        if let Some(menu_elem) = menu_ref.get() {
            let target = event.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
            if let Some(target_elem) = target {
                if !menu_elem.contains(Some(&target_elem)) {
                    set_is_open.set(false);
                }
            }
        }
    };

    leptos::create_effect(move |_| {
        if is_open.get() {
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(close_on_outside_click) as Box<dyn Fn(MouseEvent)>);
                    let _ = document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
                    closure.forget();
                }
            }
        }
    });

    view! {
        <div class="dropdown-menu-container" node_ref=menu_ref>
            <button
                class="dropdown-menu-toggle"
                on:click=move |ev| {
                    ev.stop_propagation();
                    set_is_open.update(|open| *open = !*open);
                }
                title="More actions"
            >
                <i class="fa-solid fa-ellipsis-vertical"></i>
            </button>

            {move || if is_open.get() {
                view! {
                    <div class="dropdown-menu-items">
                        {items.iter().map(|item| {
                            let on_click = item.on_click.clone();
                            view! {
                                <button
                                    class="dropdown-menu-item"
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        on_click();
                                        set_is_open.set(false);
                                    }
                                >
                                    <i class=item.icon></i>
                                    <span>{item.label}</span>
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_view()
            } else {
                view! { <div></div> }.into_view()
            }}
        </div>
    }
}
