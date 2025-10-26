use leptos::{component, view, ReadSignal, WriteSignal, IntoView, create_signal, create_memo, SignalGet, SignalUpdate, SignalSet, For, Signal, store_value, Callback, Callable, SignalWith, SignalGetUntracked};
use crate::models::{Line, RailwayGraph, GraphView, ViewportState, Routes, LineSortMode};
use crate::components::line_editor::LineEditor;
use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::components::dropdown_menu::{DropdownMenu, MenuItem};
use crate::components::line_sort_selector::LineSortSelector;
use std::collections::HashSet;
use std::rc::Rc;

fn calculate_new_sort_index(lines: &[Line], _dragged_id: uuid::Uuid, drop_target_id: uuid::Uuid) -> Option<f64> {
    // Find the dragged line and target line in the sorted list
    let target_pos = lines.iter().position(|l| l.id == drop_target_id)?;

    // Get the sort indices before and after the drop position
    let before_idx = if target_pos > 0 {
        lines[target_pos - 1].sort_index
    } else {
        None
    };
    let after_idx = lines[target_pos].sort_index;

    // Calculate midpoint between before and after
    match (before_idx, after_idx) {
        (Some(before), Some(after)) => Some((before + after) / 2.0),
        (None, Some(after)) => Some(after / 2.0),
        (Some(before), None) => Some(before + 1.0),
        (None, None) => {
            #[allow(clippy::cast_precision_loss)]
            let index = target_pos as f64;
            Some(index)
        }
    }
}

fn sort_lines(mut lines: Vec<Line>, mode: LineSortMode) -> Vec<Line> {
    match mode {
        LineSortMode::AddedOrder => {
            // Keep original order
        }
        LineSortMode::Alphabetical => {
            lines.sort_by(|a, b| a.name.cmp(&b.name));
        }
        LineSortMode::Manual => {
            // Sort by sort_index, falling back to original order for None values
            lines.sort_by(|a, b| {
                match (a.sort_index, b.sort_index) {
                    (Some(a_idx), Some(b_idx)) => a_idx.partial_cmp(&b_idx).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
    }
    lines
}

#[component]
#[must_use]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_create_view: Callback<GraphView>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
) -> impl IntoView {
    let (open_editors, set_open_editors) = create_signal(HashSet::<uuid::Uuid>::new());
    let (delete_pending, set_delete_pending) = create_signal(None::<uuid::Uuid>);

    let editors_list = move || {
        open_editors.get().into_iter().collect::<Vec<_>>()
    };

    let sorted_lines = create_memo(move |_| {
        let lines_vec = lines.get();
        let sort_mode = settings.with(|s| s.line_sort_mode);
        sort_lines(lines_vec, sort_mode)
    });

    let (drag_over_id, set_drag_over_id) = create_signal(None::<uuid::Uuid>);
    let (dragged_id, set_dragged_id) = create_signal(None::<uuid::Uuid>);

    view! {
        <div class="controls">
            <LineSortSelector settings=settings set_settings=set_settings />
            <div class="line-controls">
                <For
                    each={move || sorted_lines.get().into_iter().map(|line| line.id).collect::<Vec<_>>()}
                    key={|line_id| *line_id}
                    children={move |line_id: uuid::Uuid| {
                        view! {
                            <LineControl
                                line_id=line_id
                                lines=lines
                                set_lines=set_lines
                                graph=graph
                                set_settings=set_settings
                                dragged_id=dragged_id
                                set_dragged_id=set_dragged_id
                                drag_over_id=drag_over_id
                                set_drag_over_id=set_drag_over_id
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
    set_settings: WriteSignal<crate::models::ProjectSettings>,
    dragged_id: ReadSignal<Option<uuid::Uuid>>,
    set_dragged_id: WriteSignal<Option<uuid::Uuid>>,
    drag_over_id: ReadSignal<Option<uuid::Uuid>>,
    set_drag_over_id: WriteSignal<Option<uuid::Uuid>>,
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
                let is_dragging = move || dragged_id.get() == Some(line_id);
                let is_drag_over = move || drag_over_id.get() == Some(line_id);

                view! {
                    <div
                        class=move || {
                            let mut classes = vec!["line-control"];
                            if is_dragging() { classes.push("dragging"); }
                            if is_drag_over() { classes.push("drag-over"); }
                            classes.join(" ")
                        }
                        style=format!("border-left: 4px solid {}", line.color)
                        draggable="true"
                        on:dragstart=move |ev| {
                            if let Some(dt) = ev.data_transfer() {
                                let _ = dt.set_data("text/plain", &line_id.to_string());
                                dt.set_effect_allowed("move");
                            }
                            set_dragged_id.set(Some(line_id));
                            // Switch to Manual mode when starting drag
                            set_settings.update(|s| s.line_sort_mode = LineSortMode::Manual);
                        }
                        on:dragover=move |ev| {
                            ev.prevent_default();
                            if let Some(dt) = ev.data_transfer() {
                                dt.set_drop_effect("move");
                            }
                            set_drag_over_id.set(Some(line_id));
                        }
                        on:dragleave=move |_| {
                            set_drag_over_id.set(None);
                        }
                        on:drop=move |ev| {
                            ev.prevent_default();
                            ev.stop_propagation();

                            if let Some(dragged) = dragged_id.get_untracked() {
                                if dragged != line_id {
                                    let sorted = sort_lines(lines.get_untracked(), LineSortMode::Manual);
                                    if let Some(new_index) = calculate_new_sort_index(&sorted, dragged, line_id) {
                                        set_lines.update(|lines_vec| {
                                            if let Some(line) = lines_vec.iter_mut().find(|l| l.id == dragged) {
                                                line.sort_index = Some(new_index);
                                            }
                                        });
                                    }
                                }
                            }
                            set_dragged_id.set(None);
                            set_drag_over_id.set(None);
                        }
                        on:dragend=move |_| {
                            set_dragged_id.set(None);
                            set_drag_over_id.set(None);
                        }
                    >
                        <div
                            class="line-header"
                            on:dblclick=move |_| on_edit.with_value(|f| f(line_id))
                        >
                            <div class="drag-handle">
                                <i class="fa-solid fa-grip-vertical"></i>
                            </div>
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
                                                            source_line_id: Some(line_clone.id),
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