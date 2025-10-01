use leptos::*;
use crate::models::{Line, RailwayGraph};
use crate::components::line_editor::LineEditor;
use std::collections::HashSet;


#[component]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
) -> impl IntoView {
    let (open_editors, set_open_editors) = create_signal(HashSet::<String>::new());

    let editors_list = move || {
        open_editors.get().into_iter().collect::<Vec<_>>()
    };

    view! {
        <div class="controls">
            <h3>"Line Configuration:"</h3>
            <div class="line-controls">
                {move || {
                    lines.get().into_iter().map(|line| {
                        let line_id = line.id.clone();
                        view! {
                            <LineControl
                                line_id=line_id.clone()
                                lines=lines
                                set_lines=set_lines
                                on_edit=move |id: String| {
                                    set_open_editors.update(|editors| {
                                        editors.insert(id);
                                    });
                                }
                            />
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>

        <For
            each=editors_list
            key=|line_id| line_id.clone()
            children=move |line_id: String| {
                let current_line = Signal::derive({
                    let line_id = line_id.clone();
                    move || {
                        lines.get().into_iter().find(|l| l.id == line_id)
                    }
                });

                let is_open = Signal::derive({
                    let line_id = line_id.clone();
                    move || open_editors.get().contains(&line_id)
                });

                view! {
                    <LineEditor
                        initial_line=current_line
                        is_open=is_open
                        set_is_open={
                            let line_id = line_id.clone();
                            move |open: bool| {
                                if !open {
                                    set_open_editors.update(|editors| {
                                        editors.remove(&line_id);
                                    });
                                }
                            }
                        }
                        graph=graph
                        on_save={
                            move |edited_line: Line| {
                                set_lines.update(|lines_vec| {
                                    if let Some(line) = lines_vec.iter_mut().find(|l| l.id == edited_line.id) {
                                        *line = edited_line;
                                    }
                                });
                            }
                        }
                    />
                }
            }
        />
    }
}

#[component]
pub fn LineControl(
    line_id: String,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    on_edit: impl Fn(String) + 'static,
) -> impl IntoView {
    let id_for_derive = line_id.clone();
    let current_line = Signal::derive(move || {
        lines.get().into_iter().find(|l| l.id == id_for_derive)
    });

    let id_for_edit = line_id.clone();
    let on_edit = store_value(on_edit);

    view! {
        {move || {
            current_line.get().map(|line| {
                let id_for_edit = id_for_edit.clone();
                view! {
                    <div
                        class="line-control"
                        style=format!("border-left: 4px solid {}", line.color)
                    >
                        <div class="line-header">
                            <strong>{line_id.clone()}</strong>
                            <div class="line-header-controls">
                                <button
                                    class="visibility-toggle"
                                    on:click={
                                        let id = line_id.clone();
                                        move |_| {
                                            set_lines.update(|lines_vec| {
                                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                                    line.visible = !line.visible;
                                                }
                                            });
                                        }
                                    }
                                    title=if line.visible { "Hide line" } else { "Show line" }
                                >
                                    <i class=if line.visible { "fa-solid fa-eye" } else { "fa-solid fa-eye-slash" }></i>
                                </button>
                                <button
                                    class="edit-button"
                                    on:click=move |_| on_edit.with_value(|f| f(id_for_edit.clone()))
                                    title="Edit line"
                                >
                                    <i class="fa-solid fa-pen"></i>
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })
        }}
    }
}