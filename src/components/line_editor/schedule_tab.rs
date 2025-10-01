use super::ManualDeparturesList;
use crate::components::{
    duration_input::DurationInput,
    tab_view::TabPanel,
    time_input::TimeInput,
};
use crate::models::{Line, ScheduleMode, RailwayGraph};
use leptos::*;
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
                    {
                        let on_save = on_save.get_value();
                        move || {
                            view! {
                                <div class="form-group">
                                    <label>"Frequency"</label>
                                    <DurationInput
                                        duration=Signal::derive(move || edited_line.get().map(|l| l.frequency).unwrap_or_default())
                                        on_change={
                                            let on_save = on_save.clone();
                                            move |freq| {
                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                    updated_line.frequency = freq;
                                                    set_edited_line.set(Some(updated_line.clone()));
                                                    on_save(updated_line);
                                                }
                                            }
                                        }
                                    />
                                </div>

                                <div class="form-group">
                                    <label>"First Departure"</label>
                                    <TimeInput
                                        label=""
                                        value=Signal::derive(move || edited_line.get().map(|l| l.first_departure).unwrap_or_default())
                                        default_time="05:00"
                                        on_change={
                                            let on_save = on_save.clone();
                                            Box::new(move |time| {
                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                    updated_line.first_departure = time;
                                                    set_edited_line.set(Some(updated_line.clone()));
                                                    on_save(updated_line);
                                                }
                                            })
                                        }
                                    />
                                </div>

                                <div class="form-group">
                                    <label>"Return First Departure"</label>
                                    <TimeInput
                                        label=""
                                        value=Signal::derive(move || edited_line.get().map(|l| l.return_first_departure).unwrap_or_default())
                                        default_time="06:00"
                                        on_change={
                                            let on_save = on_save.clone();
                                            Box::new(move |time| {
                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                    updated_line.return_first_departure = time;
                                                    set_edited_line.set(Some(updated_line.clone()));
                                                    on_save(updated_line);
                                                }
                                            })
                                        }
                                    />
                                </div>
                            }
                        }
                    }
                </Show>

                <Show when=move || matches!(edited_line.get().map(|l| l.schedule_mode).unwrap_or_default(), ScheduleMode::Manual)>
                    <ManualDeparturesList
                        edited_line=edited_line
                        set_edited_line=set_edited_line
                        graph=graph
                        on_save=on_save.get_value()
                    />
                </Show>
            </div>
        </TabPanel>
    }
}
