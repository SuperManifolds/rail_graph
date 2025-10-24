use super::{ManualDeparturesList, auto_schedule_form::AutoScheduleForm};
use crate::components::tab_view::TabPanel;
use crate::models::{Line, ScheduleMode, RailwayGraph};
use leptos::{component, view, ReadSignal, WriteSignal, RwSignal, IntoView, store_value, Signal, SignalGet, event_target_checked, SignalGetUntracked, SignalSet, Show, Callback};
use std::rc::Rc;

#[component]
pub fn ScheduleTab(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: Rc<dyn Fn(Line)>,
    active_tab: RwSignal<String>,
) -> impl IntoView {
    let on_save = store_value(on_save);
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "schedule")>
            <div class="line-editor-content">
                <div class="form-group">
                    <label>
                        <input
                            type="checkbox"
                            checked=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Auto)
                            on:change={
                                let on_save = on_save.get_value();
                                move |ev| {
                                    let is_auto = event_target_checked(&ev);
                                    let mode = if is_auto { ScheduleMode::Auto } else { ScheduleMode::Manual };
                                    if let Some(mut updated_line) = edited_line.get_untracked() {
                                        updated_line.schedule_mode = mode;
                                        set_edited_line.set(Some(updated_line.clone()));
                                        on_save(updated_line);
                                    }
                                }
                            }
                        />
                        " Auto Schedule"
                    </label>
                </div>

                <Show when=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Auto)>
                    <AutoScheduleForm
                        edited_line=Signal::derive(move || edited_line.get())
                        on_update=Callback::new({
                            let on_save = on_save.get_value();
                            move |updated_line: Line| {
                                set_edited_line.set(Some(updated_line.clone()));
                                on_save(updated_line);
                            }
                        })
                    />
                </Show>

                <div class="manual-departures-section">
                    <ManualDeparturesList
                        edited_line=edited_line
                        set_edited_line=set_edited_line
                        graph=graph
                        on_save=on_save.get_value()
                    />
                </div>
            </div>
        </TabPanel>
    }
}
