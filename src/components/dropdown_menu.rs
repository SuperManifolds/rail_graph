use leptos::{component, view, IntoView, create_signal, SignalGet, SignalSet, SignalUpdate, create_node_ref, on_cleanup};
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use std::rc::Rc;
use std::cell::RefCell;

type ClickListenerClosure = wasm_bindgen::closure::Closure<dyn Fn(MouseEvent)>;
type ListenerCleanup = Rc<RefCell<Option<ClickListenerClosure>>>;

#[derive(Clone)]
pub struct MenuItem {
    pub label: &'static str,
    pub icon: &'static str,
    pub on_click: Rc<dyn Fn() + 'static>,
}

fn add_click_listener(listener_cleanup: &ListenerCleanup, callback: impl Fn(MouseEvent) + 'static) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(callback) as Box<dyn Fn(MouseEvent)>);
    let _ = document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    *listener_cleanup.borrow_mut() = Some(closure);
}

fn remove_click_listener(closure: &ClickListenerClosure) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let _ = document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
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

    let listener_cleanup: ListenerCleanup = Rc::new(RefCell::new(None));

    leptos::create_effect({
        let listener_cleanup = listener_cleanup.clone();
        move |_| {
            // Clean up previous listener if any
            if let Some(closure) = listener_cleanup.borrow_mut().take() {
                remove_click_listener(&closure);
                drop(closure);
            }

            if is_open.get() {
                add_click_listener(&listener_cleanup, close_on_outside_click);
            }
        }
    });

    // Clean up on component unmount
    on_cleanup(move || {
        if let Some(closure) = listener_cleanup.borrow_mut().take() {
            remove_click_listener(&closure);
            drop(closure);
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
