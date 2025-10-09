use leptos::*;
use crate::models::{Line, RailwayGraph};
use crate::components::line_editor::LineEditor;
use crate::components::confirmation_dialog::ConfirmationDialog;
use std::collections::HashSet;
use std::rc::Rc;


#[component]
#[must_use]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
) -> impl IntoView {
    let (open_editors, set_open_editors) = create_signal(HashSet::<String>::new());
    let (delete_pending, set_delete_pending) = create_signal(None::<String>);

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
                                on_delete=move |id: String| {
                                    set_delete_pending.set(Some(id));
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

        <ConfirmationDialog
            is_open=Signal::derive(move || delete_pending.get().is_some())
            title=Signal::derive(|| "Delete Line".to_string())
            message=Signal::derive(move || {
                delete_pending.get()
                    .map(|id| format!("Are you sure you want to delete line \"{}\"? This action cannot be undone.", id))
                    .unwrap_or_default()
            })
            on_confirm=Rc::new(move || {
                if let Some(id) = delete_pending.get() {
                    set_lines.update(|lines_vec| {
                        lines_vec.retain(|l| l.id != id);
                    });
                    set_open_editors.update(|editors| {
                        editors.remove(&id);
                    });
                    set_delete_pending.set(None);
                }
            })
            on_cancel=Rc::new(move || set_delete_pending.set(None))
            confirm_text="Delete".to_string()
        />
    }
}

#[component]
pub fn LineControl(
    line_id: String,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    on_edit: impl Fn(String) + 'static,
    on_delete: impl Fn(String) + 'static,
) -> impl IntoView {
    let id_for_derive = line_id.clone();
    let current_line = Signal::derive(move || {
        lines.get().into_iter().find(|l| l.id == id_for_derive)
    });

    let id_for_edit = line_id.clone();
    let on_edit = store_value(on_edit);
    let on_delete = store_value(on_delete);

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
                                <button
                                    class="delete-button"
                                    on:click={
                                        let id = line_id.clone();
                                        move |_| on_delete.with_value(|f| f(id.clone()))
                                    }
                                    title="Delete line"
                                >
                                    <i class="fa-solid fa-trash"></i>
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })
        }}
    }
}