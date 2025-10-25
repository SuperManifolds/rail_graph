use leptos::{component, view, ReadSignal, WriteSignal, IntoView, create_signal, SignalGet, SignalUpdate, SignalSet, For, Signal, store_value, Callback, Callable};
use crate::models::{Line, RailwayGraph, GraphView, ViewportState, Routes};
use crate::components::line_editor::LineEditor;
use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::components::dropdown_menu::{DropdownMenu, MenuItem};
use std::collections::HashSet;
use std::rc::Rc;


#[component]
#[must_use]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_create_view: Callback<GraphView>,
    settings: ReadSignal<crate::models::ProjectSettings>,
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
                <For
                    each={move || lines.get().into_iter().map(|line| line.id).collect::<Vec<_>>()}
                    key={|line_id| *line_id}
                    children={move |line_id: uuid::Uuid| {
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
                                on_duplicate=move |id: uuid::Uuid| {
                                    set_lines.update(|lines_vec| {
                                        if let Some(line) = lines_vec.iter().find(|l| l.id == id) {
                                            let duplicated = line.duplicate();
                                            lines_vec.push(duplicated);
                                        }
                                    });
                                }
                                on_create_view=on_create_view
                            />
                        }
                    }}
                />
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
                        settings=settings
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
    on_duplicate: impl Fn(uuid::Uuid) + 'static,
    on_create_view: Callback<GraphView>,
) -> impl IntoView {
    let current_line = Signal::derive(move || {
        lines.get().into_iter().find(|l| l.id == line_id)
    });

    let on_edit = store_value(on_edit);
    let on_delete = store_value(on_delete);
    let on_duplicate = store_value(on_duplicate);

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
                                <DropdownMenu items={
                                    let line_clone = line.clone();
                                    vec![
                                        MenuItem {
                                            label: "Open in View",
                                            icon: "fa-solid fa-arrow-up-right-from-square",
                                            on_click: Rc::new(move || {
                                                use crate::models::RouteDirection;

                                                let edge_path: Vec<usize> = line_clone.forward_route.iter().map(|seg| seg.edge_index).collect();
                                                if !edge_path.is_empty() {
                                                    let current_graph = graph.get();
                                                    let (from, to) = current_graph.get_route_endpoints(&line_clone.forward_route, RouteDirection::Forward);

                                                    if let (Some(from), Some(to)) = (from, to) {
                                                        let view = GraphView {
                                                            id: uuid::Uuid::new_v4(),
                                                            name: line_clone.name.clone(),
                                                            viewport_state: ViewportState::default(),
                                                            station_range: Some((from, to)),
                                                            edge_path: Some(edge_path),
                                                        };
                                                        on_create_view.call(view);
                                                    }
                                                }
                                            }),
                                        },
                                        MenuItem {
                                            label: "Edit",
                                            icon: "fa-solid fa-pen",
                                            on_click: Rc::new(move || on_edit.with_value(|f| f(line_id))),
                                        },
                                        MenuItem {
                                            label: "Duplicate",
                                            icon: "fa-solid fa-copy",
                                            on_click: Rc::new(move || on_duplicate.with_value(|f| f(line_id))),
                                        },
                                        MenuItem {
                                            label: "Delete",
                                            icon: "fa-solid fa-trash",
                                            on_click: Rc::new(move || on_delete.with_value(|f| f(line_id))),
                                        },
                                    ]
                                } />
                            </div>
                        </div>
                    </div>
                }
            })
        }}
    }
}