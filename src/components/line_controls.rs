use leptos::{component, view, ReadSignal, WriteSignal, IntoView, create_signal, SignalGet, SignalUpdate, SignalSet, For, Signal, store_value, Callback, Callable};
use crate::models::{Line, RailwayGraph, GraphView};
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
    on_create_view: Callback<GraphView>,
) -> impl IntoView {
    let (open_editors, set_open_editors) = create_signal(HashSet::<uuid::Uuid>::new());
    let (delete_pending, set_delete_pending) = create_signal(None::<uuid::Uuid>);

    let editors_list = move || {
        open_editors.get().into_iter().collect::<Vec<_>>()
    };

    view! {
        <div class="controls">
            <h3>"Line Configuration:"</h3>
            <div class="line-controls">
                {move || {
                    lines.get().into_iter().map(|line| {
                        let line_id = line.id;
                        view! {
                            <LineControl
                                line_id=line_id
                                lines=lines
                                set_lines=set_lines
                                graph=graph
                                on_edit=move |id: uuid::Uuid| {
                                    set_open_editors.update(|editors| {
                                        editors.insert(id);
                                    });
                                }
                                on_delete=move |id: uuid::Uuid| {
                                    set_delete_pending.set(Some(id));
                                }
                                on_create_view=on_create_view
                            />
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>

        <For
            each=editors_list
            key=|line_id| *line_id
            children=move |line_id: uuid::Uuid| {
                let current_line = Signal::derive({
                    move || {
                        lines.get().into_iter().find(|l| l.id == line_id)
                    }
                });

                let is_open = Signal::derive({
                    move || open_editors.get().contains(&line_id)
                });

                view! {
                    <LineEditor
                        initial_line=current_line
                        is_open=is_open
                        set_is_open={
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
                    .and_then(|id| lines.get().into_iter().find(|l| l.id == id))
                    .map(|line| format!("Are you sure you want to delete line \"{}\"? This action cannot be undone.", line.name))
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
    line_id: uuid::Uuid,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_edit: impl Fn(uuid::Uuid) + 'static,
    on_delete: impl Fn(uuid::Uuid) + 'static,
    on_create_view: Callback<GraphView>,
) -> impl IntoView {
    let current_line = Signal::derive(move || {
        lines.get().into_iter().find(|l| l.id == line_id)
    });

    let on_edit = store_value(on_edit);
    let on_delete = store_value(on_delete);

    view! {
        {move || {
            current_line.get().map(|line| {
                view! {
                    <div
                        class="line-control"
                        style=format!("border-left: 4px solid {}", line.color)
                    >
                        <div class="line-header">
                            <strong>{line.name.clone()}</strong>
                            <div class="line-header-controls">
                                <button
                                    class="visibility-toggle"
                                    on:click={
                                        move |_| {
                                            set_lines.update(|lines_vec| {
                                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == line_id) {
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
                                    class="create-view-button"
                                    on:click={
                                        let line = line.clone();
                                        let current_graph = graph.get();
                                        move |_| {
                                            // Use the line's forward route to create the view
                                            let edge_path: Vec<usize> = line.forward_route.iter().map(|seg| seg.edge_index).collect();
                                            if !edge_path.is_empty() {
                                                if let Ok(view) = GraphView::from_edge_path(line.name.clone(), edge_path, &current_graph) {
                                                    on_create_view.call(view);
                                                }
                                            }
                                        }
                                    }
                                    title="Open line in new view"
                                >
                                    <i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </button>
                                <button
                                    class="edit-button"
                                    on:click=move |_| on_edit.with_value(|f| f(line_id))
                                    title="Edit line"
                                >
                                    <i class="fa-solid fa-pen"></i>
                                </button>
                                <button
                                    class="delete-button"
                                    on:click={
                                        move |_| on_delete.with_value(|f| f(line_id))
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