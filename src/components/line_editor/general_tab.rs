use crate::components::tab_view::TabPanel;
use crate::models::Line;
use leptos::*;
use std::rc::Rc;

#[component]
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
                        value=move || edited_line.get().map(|l| l.id.clone()).unwrap_or_default()
                        on:change={
                            let on_save = on_save.get_value();
                            move |ev| {
                                let name = event_target_value(&ev);
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    updated_line.id = name;
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
            </div>
        </TabPanel>
    }
}
