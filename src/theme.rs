use leptos::{create_signal, create_effect, on_cleanup, ReadSignal, SignalSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

/// Hook that provides reactive theme state based on system preferences
///
/// # Panics
///
/// Panics if the browser window or media query API is not available.
#[must_use]
pub fn use_theme() -> ReadSignal<Theme> {
    let (theme, set_theme) = create_signal(Theme::Dark);

    create_effect(move |_| {
        let window = web_sys::window().expect("window");
        let media_query = window
            .match_media("(prefers-color-scheme: dark)")
            .expect("matchMedia")
            .expect("media query list");

        // Set initial theme
        let is_dark = media_query.matches();
        let initial_theme = if is_dark { Theme::Dark } else { Theme::Light };
        set_theme.set(initial_theme);

        // Create event listener for theme changes
        let closure = Closure::wrap(Box::new(move |event: wasm_bindgen::JsValue| {
            let Ok(matches) = js_sys::Reflect::get(&event, &"matches".into()) else { return };
            let Some(is_dark) = matches.as_bool() else { return };
            let new_theme = if is_dark { Theme::Dark } else { Theme::Light };
            set_theme.set(new_theme);
        }) as Box<dyn FnMut(_)>);

        // Add listener
        media_query
            .add_listener_with_opt_callback(Some(closure.as_ref().unchecked_ref()))
            .expect("add listener");

        // Clean up listener on component unmount
        on_cleanup(move || {
            let _ = media_query.remove_listener_with_opt_callback(Some(closure.as_ref().unchecked_ref()));
            closure.forget();
        });
    });

    theme
}
