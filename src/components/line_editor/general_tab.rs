use crate::components::tab_view::TabPanel;
use crate::components::duration_input::DurationInput;
use crate::models::{Line, LineStyle};
use leptos::{component, view, ReadSignal, WriteSignal, RwSignal, IntoView, store_value, Signal, SignalGet, event_target_value, event_target_checked, SignalGetUntracked, SignalSet, Show};
use std::rc::Rc;

/// Check if line view feature is enabled via localStorage
fn is_line_view_enabled() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item("enable_line_view").ok().flatten())
        .is_some()
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn GeneralTab(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
    active_tab: RwSignal<String>,
) -> impl IntoView {
    let on_save = store_value(on_save);
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "general")>
            <div class="line-editor-content">
                <div class="form-group">
                    <label>"Name"</label>
                    <input
                        type="text"
                        class="line-name-input"
                        value=move || edited_line.get().map(|l| l.name.clone()).unwrap_or_default()
                        on:change={
                            let on_save = on_save.get_value();
                            move |ev| {
                                let name = event_target_value(&ev);
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    updated_line.name = name;
                                    set_edited_line.set(Some(updated_line.clone()));
                                    on_save(updated_line);
                                }
                            }
                        }
                    />
                </div>

                <div class="form-group">
                    <label>"Code"</label>
                    <input
                        type="text"
                        class="line-code-input"
                        value=move || edited_line.get().map(|l| l.code.clone()).unwrap_or_default()
                        on:change={
                            let on_save = on_save.get_value();
                            move |ev| {
                                let code = event_target_value(&ev);
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    updated_line.code = code;
                                    set_edited_line.set(Some(updated_line.clone()));
                                    on_save(updated_line);
                                }
                            }
                        }
                    />
                </div>

                <div class="form-group">
                    <label>"Color"</label>
                    <input
                        type="color"
                        value=move || edited_line.get().map(|l| l.color).unwrap_or_default()
                        on:change={
                            let on_save = on_save.get_value();
                            move |ev| {
                                let color = event_target_value(&ev);
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    updated_line.color = color;
                                    set_edited_line.set(Some(updated_line.clone()));
                                    on_save(updated_line);
                                }
                            }
                        }
                    />
                </div>

                <div class="form-group">
                    <label>"Line Thickness"</label>
                    <div class="thickness-control">
                        {move || {
                            let current_thickness = edited_line.get().map_or(2.0, |l| l.thickness);
                            view! {
                                <input
                                    type="range"
                                    min="0.5"
                                    max="8.0"
                                    step="0.25"
                                    value=current_thickness
                                    on:change={
                                        let on_save = on_save.get_value();
                                        move |ev| {
                                            let thickness = event_target_value(&ev).parse::<f64>().unwrap_or(2.0);
                                            if let Some(mut updated_line) = edited_line.get_untracked() {
                                                updated_line.thickness = thickness;
                                                set_edited_line.set(Some(updated_line.clone()));
                                                on_save(updated_line);
                                            }
                                        }
                                    }
                                />
                                <span class="thickness-value">
                                    {format!("{current_thickness:.2}")}
                                </span>
                            }
                        }}
                    </div>
                </div>

                <Show when=is_line_view_enabled>
                    <div class="form-group">
                        <label>"Line Style"</label>
                        <select
                            value=move || {
                                edited_line.get().map_or("Solid".to_string(), |l| {
                                    match l.style {
                                        LineStyle::Solid => "Solid",
                                        LineStyle::Double => "Double",
                                        LineStyle::CenterLined => "CenterLined",
                                    }.to_string()
                                })
                            }
                            on:change={
                                let on_save = on_save.get_value();
                                move |ev| {
                                    let style_str = event_target_value(&ev);
                                    let style = match style_str.as_str() {
                                        "Double" => LineStyle::Double,
                                        "CenterLined" => LineStyle::CenterLined,
                                        _ => LineStyle::Solid,
                                    };
                                    if let Some(mut updated_line) = edited_line.get_untracked() {
                                        updated_line.style = style;
                                        set_edited_line.set(Some(updated_line.clone()));
                                        on_save(updated_line);
                                    }
                                }
                            }
                        >
                            <option value="Solid">"Solid"</option>
                            <option value="Double">"Double track"</option>
                            <option value="CenterLined">"Center line"</option>
                        </select>
                    </div>
                </Show>

                <div class="form-group">
                    <label>"Default Wait Time"</label>
                    <DurationInput
                        duration=Signal::derive(move || edited_line.get().map(|l| l.default_wait_time).unwrap_or_default())
                        on_change={
                            let on_save = on_save.get_value();
                            move |new_duration| {
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    updated_line.default_wait_time = new_duration;
                                    set_edited_line.set(Some(updated_line.clone()));
                                    on_save(updated_line);
                                }
                            }
                        }
                    />
                    <p class="form-help">"Default wait time used when adding new stops to this line"</p>
                </div>

                <div class="form-group">
                    <label class="checkbox-label">
                        <input
                            type="checkbox"
                            checked=move || edited_line.get().is_none_or(|l| l.sync_routes)
                            on:change={
                                let on_save = on_save.get_value();
                                move |ev| {
                                    let checked = event_target_checked(&ev);
                                    if let Some(mut updated_line) = edited_line.get_untracked() {
                                        updated_line.sync_routes = checked;
                                        set_edited_line.set(Some(updated_line.clone()));
                                        on_save(updated_line);
                                    }
                                }
                            }
                        />
                        "Keep forward and return routes in sync"
                    </label>
                    <p class="form-help">"When enabled, changes to forward route automatically update return route"</p>
                </div>
            </div>
        </TabPanel>
    }
}
