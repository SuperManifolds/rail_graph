use leptos::{ReadSignal, WriteSignal, SignalGet, SignalSet};
use crate::models::GraphView;
use crate::components::app::AppTab;
use wasm_bindgen::JsCast;

/// Setup keyboard shortcuts for tab switching (1-0 keys)
pub fn setup_tab_switching(
    is_capturing_shortcut: ReadSignal<bool>,
    views: ReadSignal<Vec<GraphView>>,
    set_active_tab: WriteSignal<AppTab>,
) {
    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::keydown, move |ev| {
        // Don't handle shortcuts when capturing in the shortcuts editor
        if is_capturing_shortcut.get() {
            return;
        }

        // Don't handle keyboard shortcuts when typing in input fields
        let Some(target) = ev.target() else { return };
        let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() else { return };
        let tag_name = element.tag_name().to_lowercase();
        if tag_name == "input" || tag_name == "textarea" {
            return;
        }

        // Ignore repeat events
        if ev.repeat() {
            return;
        }

        // Only handle number keys without modifiers
        if ev.ctrl_key() || ev.shift_key() || ev.alt_key() || ev.meta_key() {
            return;
        }

        // Parse digit keys (1-9, 0)
        let tab_index = match ev.code().as_str() {
            "Digit1" => Some(0), // Infrastructure tab
            "Digit2" => Some(1), // First view
            "Digit3" => Some(2),
            "Digit4" => Some(3),
            "Digit5" => Some(4),
            "Digit6" => Some(5),
            "Digit7" => Some(6),
            "Digit8" => Some(7),
            "Digit9" => Some(8),
            "Digit0" => Some(9), // 10th tab (9th view)
            _ => None,
        };

        if let Some(index) = tab_index {
            if index == 0 {
                // Switch to Infrastructure tab
                ev.prevent_default();
                set_active_tab.set(AppTab::Infrastructure);
            } else {
                // Switch to view tab (index - 1 in the views array)
                let current_views = views.get();
                if let Some(view) = current_views.get(index - 1) {
                    ev.prevent_default();
                    set_active_tab.set(AppTab::GraphView(view.id));
                }
            }
        }
    });
}
