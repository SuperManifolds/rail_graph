use leptos::{component, view, IntoView, create_signal, SignalGet, SignalSet, SignalGetUntracked, create_node_ref, on_cleanup, Portal, html, store_value, StoredValue};
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use std::rc::Rc;
use std::cell::RefCell;

const MENU_ESTIMATED_WIDTH: f64 = 160.0;
const MENU_ESTIMATED_HEIGHT: f64 = 150.0;
const MENU_SPACING: f64 = 4.0;

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
    let (menu_position, set_menu_position) = create_signal((0.0, 0.0));
    let container_ref = create_node_ref::<html::Div>();
    let button_ref = create_node_ref::<html::Button>();
    let menu_ref = create_node_ref::<html::Div>();
    let items: StoredValue<Vec<MenuItem>> = store_value(items);

    // Close menu when clicking outside
    let close_on_outside_click = move |event: MouseEvent| {
        // Check both the container (button) and the portal menu
        let target = event.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
        let Some(target_elem) = target else { return };

        let in_container = container_ref.get()
            .is_some_and(|el| el.contains(Some(&target_elem)));
        let in_menu = menu_ref.get()
            .is_some_and(|el| el.contains(Some(&target_elem)));

        if !in_container && !in_menu {
            set_is_open.set(false);
        }
    };

    let toggle_menu = move |ev: MouseEvent| {
        ev.stop_propagation();
        let new_state = !is_open.get_untracked();

        if new_state {
            // Calculate position when opening
            if let Some(button_el) = button_ref.get() {
                let rect = button_el.get_bounding_client_rect();
                let button_right = rect.right();
                let button_bottom = rect.bottom();
                let button_top = rect.top();

                // Get viewport dimensions
                let viewport_width = web_sys::window()
                    .and_then(|w| w.inner_width().ok())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1920.0);
                let viewport_height = web_sys::window()
                    .and_then(|w| w.inner_height().ok())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1080.0);

                // X: align right edge of menu with right edge of button
                let x = (button_right - MENU_ESTIMATED_WIDTH).max(MENU_SPACING);

                // Y: prefer below button, but flip above if not enough space
                let y = if button_bottom + MENU_SPACING + MENU_ESTIMATED_HEIGHT <= viewport_height {
                    button_bottom + MENU_SPACING
                } else if button_top - MENU_SPACING - MENU_ESTIMATED_HEIGHT >= 0.0 {
                    button_top - MENU_SPACING - MENU_ESTIMATED_HEIGHT
                } else {
                    // Not enough space either way, just position below
                    button_bottom + MENU_SPACING
                };

                set_menu_position.set((x.min(viewport_width - MENU_ESTIMATED_WIDTH - MENU_SPACING), y));
            }
        }

        set_is_open.set(new_state);
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
        <div class="dropdown-menu-container" node_ref=container_ref>
            <button
                class="dropdown-menu-toggle"
                on:click=toggle_menu
                title="More actions"
                node_ref=button_ref
            >
                <i class="fa-solid fa-ellipsis-vertical"></i>
            </button>

            {move || if is_open.get() {
                let (x, y) = menu_position.get();
                view! {
                    <Portal>
                        <div
                            class="dropdown-menu-items"
                            node_ref=menu_ref
                            style=format!("position: fixed; left: {x}px; top: {y}px; z-index: 9999;")
                        >
                            {items.get_value().into_iter().map(|item| {
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
                    </Portal>
                }.into_view()
            } else {
                view! {}.into_view()
            }}
        </div>
    }
}
