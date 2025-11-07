use leptos::{component, view, MaybeSignal, IntoView, Show, SignalGet, SignalWith, use_context, ReadSignal, WriteSignal};
use crate::models::UserSettings;

/// Render a keyboard key as a key cap
#[component]
#[must_use]
fn KeyCap(
    /// The key text to display
    text: &'static str,
) -> impl IntoView {
    view! {
        <span class="key-cap">{text}</span>
    }
}

#[component]
#[must_use]
pub fn CanvasControlsHint(
    /// Whether the hint should be visible
    #[prop(into)]
    visible: MaybeSignal<bool>,
    /// Whether to show horizontal scaling hint (for graph view)
    #[prop(optional)]
    show_horizontal_scaling: bool,
) -> impl IntoView {
    // Get user settings from context to read keyboard shortcuts
    let (user_settings, _) = use_context::<(ReadSignal<UserSettings>, WriteSignal<UserSettings>)>()
        .expect("UserSettings context not found");

    // Detect platform for key formatting
    let is_mac = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .is_some_and(|ua| ua.contains("Mac"));

    let is_windows = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .is_some_and(|ua| ua.contains("Windows"));

    // Helper to get formatted shortcut or default
    let get_shortcut_text = move |id: &str| -> String {
        user_settings.with(|settings| {
            settings.keyboard_shortcuts.shortcuts
                .get(id)
                .and_then(|opt| opt.as_ref())
                .map_or_else(
                    || "?".to_string(),
                    |shortcut| shortcut.format(is_mac, is_windows)
                )
        })
    };

    view! {
        <Show when=move || visible.get()>
            <div class="canvas-controls-hint">
                <div class="hint-line">
                    "Pan: "
                    {move || {
                        let text = get_shortcut_text("pan_toggle");
                        view! { <KeyCap text=Box::leak(text.into_boxed_str()) /> }
                    }}
                    " or "
                    {move || {
                        let w = get_shortcut_text("pan_up");
                        let a = get_shortcut_text("pan_left");
                        let s = get_shortcut_text("pan_down");
                        let d = get_shortcut_text("pan_right");
                        view! {
                            <KeyCap text=Box::leak(w.into_boxed_str()) />
                            <KeyCap text=Box::leak(a.into_boxed_str()) />
                            <KeyCap text=Box::leak(s.into_boxed_str()) />
                            <KeyCap text=Box::leak(d.into_boxed_str()) />
                        }
                    }}
                </div>
                <div class="hint-line">
                    "Zoom: "
                    <KeyCap text="Scroll" />
                    " or "
                    {move || {
                        let zoom_out = get_shortcut_text("zoom_out");
                        let zoom_in = get_shortcut_text("zoom_in");
                        view! {
                            <KeyCap text=Box::leak(zoom_out.into_boxed_str()) />
                            " / "
                            <KeyCap text=Box::leak(zoom_in.into_boxed_str()) />
                        }
                    }}
                </div>
                <div class="hint-line">
                    "Reset view: "
                    {move || {
                        let reset = get_shortcut_text("reset_view");
                        view! { <KeyCap text=Box::leak(reset.into_boxed_str()) /> }
                    }}
                </div>
                <Show when=move || show_horizontal_scaling>
                    <div class="hint-line">
                        "Horizontal scale: "
                        {move || {
                            let scale_out = get_shortcut_text("horizontal_scale_decrease");
                            let scale_in = get_shortcut_text("horizontal_scale_increase");
                            view! {
                                <KeyCap text=Box::leak(scale_out.into_boxed_str()) />
                                " / "
                                <KeyCap text=Box::leak(scale_in.into_boxed_str()) />
                            }
                        }}
                    </div>
                </Show>
            </div>
        </Show>
    }
}
