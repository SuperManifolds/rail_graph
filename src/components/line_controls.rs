use leptos::{component, view, ReadSignal, WriteSignal, IntoView, create_signal, create_memo, SignalGet, SignalUpdate, SignalSet, For, Signal, Callback, Callable, SignalWith, SignalGetUntracked, event_target_value};
use crate::models::{Line, LineFolder, RailwayGraph, GraphView, LineSortMode};
use crate::components::line_editor::LineEditor;
use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::components::delete_folder_confirmation::DeleteFolderConfirmation;
use crate::components::edit_folder_dialog::EditFolderDialog;
use crate::components::line_sort_selector::LineSortSelector;
use crate::components::window::Window;
use crate::components::button::Button;
use crate::components::tree_item::{TreeItem, DraggedItem, DropZone, find_item_context, build_tree};
use std::collections::HashSet;
use std::rc::Rc;

fn initialize_sort_indices_recursive(
    items: &[TreeItem],
    set_lines: WriteSignal<Vec<Line>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
) {
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::excessive_nesting)]
    for (index, item) in items.iter().enumerate() {
        let new_index = index as f64;

        match item {
            TreeItem::Line(line) if line.sort_index.is_none() => {
                let line_id = line.id;
                set_lines.update(|lines_vec| {
                    let Some(l) = lines_vec.iter_mut().find(|l| l.id == line_id) else { return };
                    l.sort_index = Some(new_index);
                });
            }
            TreeItem::Folder { folder, children } => {
                if folder.sort_index.is_none() {
                    let folder_id = folder.id;
                    set_folders.update(move |folders_vec| {
                        if let Some(f) = folders_vec.iter_mut().find(|f| f.id == folder_id) {
                            f.sort_index = Some(new_index);
                        }
                    });
                }
                // Recursively initialize children
                initialize_sort_indices_recursive(children, set_lines, set_folders);
            }
            TreeItem::Line(_) => {}
        }
    }
}

fn is_ancestor(
    folder_id: uuid::Uuid,
    potential_ancestor_id: uuid::Uuid,
    folders: &[LineFolder],
) -> bool {
    let mut current_id = Some(folder_id);
    let mut visited = std::collections::HashSet::new();

    while let Some(id) = current_id {
        if id == potential_ancestor_id {
            return true;
        }

        if !visited.insert(id) {
            // Cycle detected
            return false;
        }

        current_id = folders.iter()
            .find(|f| f.id == id)
            .and_then(|f| f.parent_folder_id);
    }

    false
}

pub fn handle_drop_into_folder(
    dragged: DraggedItem,
    folder_id: uuid::Uuid,
    set_lines: WriteSignal<Vec<Line>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
) {
    match dragged {
        DraggedItem::Line(dragged_line_id) => {
            set_lines.update(|lines_vec| {
                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == dragged_line_id) {
                    line.folder_id = Some(folder_id);
                    line.sort_index = None;
                }
            });
        }
        DraggedItem::Folder(dragged_folder_id) => {
            if dragged_folder_id == folder_id {
                return;
            }

            set_folders.update(|folders_vec| {
                // Check if target folder is a descendant of dragged folder
                if is_ancestor(folder_id, dragged_folder_id, folders_vec) {
                    return;
                }

                if let Some(f) = folders_vec.iter_mut().find(|f| f.id == dragged_folder_id) {
                    f.parent_folder_id = Some(folder_id);
                    f.sort_index = None;
                }
            });
        }
    }
}

pub fn handle_drop_in_zone(
    dragged: DraggedItem,
    drop_zone: DropZone,
    tree_items: Vec<TreeItem>,
    set_lines: WriteSignal<Vec<Line>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
) {
    let reference_id = match drop_zone {
        DropZone::Before(id) | DropZone::After(id) => id,
    };

    // Find the reference item and its context (siblings and parent folder)
    let Some((siblings, folder_id)) = find_item_context(&tree_items, reference_id, None) else {
        return;
    };

    // Find the position in the siblings list
    let reference_index = siblings.iter().position(|item| item.id() == reference_id);
    let Some(reference_index) = reference_index else { return };

    // Calculate the new sort_index based on neighbors
    // All items should have sort_index in Manual mode
    let new_sort_index = match drop_zone {
        DropZone::Before(_) => {
            if reference_index > 0 {
                let prev = &siblings[reference_index - 1];
                let curr = &siblings[reference_index];
                let prev_idx = prev.sort_index().unwrap_or(0.0);
                let curr_idx = curr.sort_index().unwrap_or(1.0);
                Some((prev_idx + curr_idx) / 2.0)
            } else {
                // First item - use value before first item's index
                let first_idx = siblings[0].sort_index().unwrap_or(0.0);
                Some(first_idx - 1.0)
            }
        }
        DropZone::After(_) => {
            if reference_index < siblings.len() - 1 {
                let curr = &siblings[reference_index];
                let next = &siblings[reference_index + 1];
                let curr_idx = curr.sort_index().unwrap_or(0.0);
                let next_idx = next.sort_index().unwrap_or(1.0);
                Some((curr_idx + next_idx) / 2.0)
            } else {
                // Last item - use value after last item's index
                let last_idx = siblings[reference_index].sort_index().unwrap_or(0.0);
                Some(last_idx + 1.0)
            }
        }
    };

    // Update the dragged item
    match dragged {
        DraggedItem::Line(dragged_line_id) => {
            set_lines.update(|lines_vec| {
                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == dragged_line_id) {
                    line.folder_id = folder_id;
                    line.sort_index = new_sort_index;
                }
            });
        }
        DraggedItem::Folder(dragged_folder_id) => {
            set_folders.update(|folders_vec| {
                // If target has a folder, check if it's a descendant of dragged folder
                if let Some(target_folder_id) = folder_id {
                    if is_ancestor(target_folder_id, dragged_folder_id, folders_vec) {
                        return;
                    }
                }

                if let Some(f) = folders_vec.iter_mut().find(|f| f.id == dragged_folder_id) {
                    f.parent_folder_id = folder_id;
                    f.sort_index = new_sort_index;
                }
            });
        }
    }
}

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    #[allow(unused_variables)]
    folders: ReadSignal<Vec<LineFolder>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
    graph: ReadSignal<RailwayGraph>,
    on_create_view: Callback<GraphView>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
    on_line_editor_opened: Callback<uuid::Uuid>,
    on_line_editor_closed: Callback<uuid::Uuid>,
) -> impl IntoView {
    let (open_editors, set_open_editors) = create_signal(HashSet::<uuid::Uuid>::new());
    let (delete_pending, set_delete_pending) = create_signal(None::<uuid::Uuid>);
    let (folder_delete_pending, set_folder_delete_pending) = create_signal(None::<uuid::Uuid>);
    let (folder_edit_pending, set_folder_edit_pending) = create_signal(None::<uuid::Uuid>);

    let editors_list = move || {
        open_editors.get().into_iter().collect::<Vec<_>>()
    };

    let (drag_over_id, set_drag_over_id) = create_signal(None::<uuid::Uuid>);
    let (dragged_item, set_dragged_item) = create_signal(None::<DraggedItem>);
    let (drop_zone_hover, set_drop_zone_hover) = create_signal(None::<DropZone>);
    let (show_folder_dialog, set_show_folder_dialog) = create_signal(false);
    let (folder_name, set_folder_name) = create_signal(String::from("New Folder"));
    let (folder_color, set_folder_color) = create_signal(String::from("#808080"));

    let tree = create_memo(move |_| {
        let lines_vec = lines.get();
        let folders_vec = folders.get();
        let sort_mode = settings.with(|s| s.line_sort_mode);
        build_tree(lines_vec, folders_vec, sort_mode)
    });

    // Initialize sort indices when switching to Manual mode
    leptos::create_effect(move |prev_mode| {
        let current_mode = settings.with(|s| s.line_sort_mode);
        if current_mode == LineSortMode::Manual && prev_mode != Some(LineSortMode::Manual) {
            let tree_items = tree.get_untracked();
            initialize_sort_indices_recursive(&tree_items, set_lines, set_folders);
        }
        current_mode
    });

    view! {
        <div class="controls">
            <div class="controls-header">
                <LineSortSelector settings=settings set_settings=set_settings />
                <Button
                    class="add-folder-button"
                    on_click=Callback::new(move |_| set_show_folder_dialog.set(true))
                    title="Create new folder"
                >
                    <i class="fa-solid fa-folder-plus"></i>
                </Button>
            </div>
            <div class="line-controls"
                on:dragover=move |ev| {
                    ev.prevent_default();
                    if let Some(dt) = ev.data_transfer() {
                        dt.set_drop_effect("move");
                    }
                }
                on:drop=move |ev| {
                    ev.prevent_default();

                    if let Some(dragged) = dragged_item.get_untracked() {
                        match dragged {
                            DraggedItem::Line(dragged_line_id) => {
                                set_lines.update(|lines_vec| {
                                    if let Some(line) = lines_vec.iter_mut().find(|l| l.id == dragged_line_id) {
                                        line.folder_id = None;
                                        line.sort_index = None;
                                    }
                                });
                            }
                            DraggedItem::Folder(dragged_folder_id) => {
                                set_folders.update(|folders_vec| {
                                    if let Some(f) = folders_vec.iter_mut().find(|f| f.id == dragged_folder_id) {
                                        f.parent_folder_id = None;
                                        f.sort_index = None;
                                    }
                                });
                            }
                        }
                    }
                    set_dragged_item.set(None);
                    set_drag_over_id.set(None);
                }
            >
                <For
                    each={move || {
                        let items = tree.get();
                        let len = items.len();
                        items.into_iter().enumerate().map(|(idx, item)| (item, idx == len - 1)).collect::<Vec<_>>()
                    }}
                    key={|(item, _)| item.id()}
                    children={move |(tree_item, is_last): (TreeItem, bool)| {
                        view! {
                            <TreeItem
                                tree_item=tree_item
                                is_last=is_last
                                tree=tree
                                lines=lines
                                set_lines=set_lines
                                folders=folders
                                set_folders=set_folders
                                graph=graph
                                settings=settings
                                set_settings=set_settings
                                dragged_item=dragged_item
                                set_dragged_item=set_dragged_item
                                drag_over_id=drag_over_id
                                set_drag_over_id=set_drag_over_id
                                drop_zone_hover=drop_zone_hover
                                set_drop_zone_hover=set_drop_zone_hover
                                on_edit=move |id: uuid::Uuid| {
                                    set_open_editors.update(|editors| {
                                        editors.insert(id);
                                    });
                                    on_line_editor_opened.call(id);
                                }
                                on_delete=move |id: uuid::Uuid| {
                                    set_delete_pending.set(Some(id));
                                }
                                on_duplicate=move |id: uuid::Uuid| {
                                    set_lines.update(|lines_vec| {
                                        if let Some(line) = lines_vec.iter().find(|l| l.id == id) {
                                            let mut duplicated = line.duplicate();
                                            // Assign sort_index if in Manual mode
                                            if settings.with(|s| s.line_sort_mode == LineSortMode::Manual) {
                                                #[allow(clippy::cast_precision_loss)]
                                                let max_sort_index = lines_vec
                                                    .iter()
                                                    .filter_map(|l| l.sort_index)
                                                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                                    .unwrap_or(-1.0);
                                                duplicated.sort_index = Some(max_sort_index + 1.0);
                                            }
                                            lines_vec.push(duplicated);
                                        }
                                    });
                                }
                                on_folder_edit=move |id: uuid::Uuid| {
                                    set_folder_edit_pending.set(Some(id));
                                }
                                on_folder_delete=move |id: uuid::Uuid| {
                                    set_folder_delete_pending.set(Some(id));
                                }
                                on_create_view=on_create_view
                                depth=0
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
                                    on_line_editor_closed.call(line_id);
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
                        if editors.remove(&id) {
                            // Editor was open, notify parent
                            on_line_editor_closed.call(id);
                        }
                    });
                    set_delete_pending.set(None);
                }
            })
            on_cancel=Rc::new(move || set_delete_pending.set(None))
            confirm_text="Delete".to_string()
        />

        <Window
            is_open=Signal::derive(move || show_folder_dialog.get())
            title=Signal::derive(|| "Create Folder".to_string())
            on_close=move || set_show_folder_dialog.set(false)
            max_size=(400.0, 300.0)
        >
            <div class="add-station-form">
                <div class="form-field">
                    <label>"Folder Name"</label>
                    <input
                        type="text"
                        placeholder="Enter folder name"
                        value=folder_name
                        on:input=move |ev| set_folder_name.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" && !folder_name.get().trim().is_empty() {
                                let mut new_folder = LineFolder::new(folder_name.get(), folder_color.get());
                                set_folders.update(|folders_vec| {
                                    // Assign sort_index if in Manual mode
                                    if settings.with(|s| s.line_sort_mode == LineSortMode::Manual) {
                                        #[allow(clippy::cast_precision_loss)]
                                        let max_sort_index = folders_vec
                                            .iter()
                                            .filter_map(|f| f.sort_index)
                                            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                            .unwrap_or(-1.0);
                                        new_folder.sort_index = Some(max_sort_index + 1.0);
                                    }
                                    folders_vec.push(new_folder);
                                });
                                set_folder_name.set(String::from("New Folder"));
                                set_folder_color.set(String::from("#808080"));
                                set_show_folder_dialog.set(false);
                            }
                        }
                        prop:autofocus=true
                    />
                </div>

                <div class="form-field">
                    <label>"Color"</label>
                    <input
                        type="color"
                        value=folder_color
                        on:input=move |ev| set_folder_color.set(event_target_value(&ev))
                    />
                </div>

                <div class="form-buttons">
                    <button on:click=move |_| set_show_folder_dialog.set(false)>
                        "Cancel"
                    </button>
                    <button
                        class="primary"
                        on:click=move |_| {
                            if !folder_name.get().trim().is_empty() {
                                let mut new_folder = LineFolder::new(folder_name.get(), folder_color.get());
                                set_folders.update(|folders_vec| {
                                    // Assign sort_index if in Manual mode
                                    if settings.with(|s| s.line_sort_mode == LineSortMode::Manual) {
                                        #[allow(clippy::cast_precision_loss)]
                                        let max_sort_index = folders_vec
                                            .iter()
                                            .filter_map(|f| f.sort_index)
                                            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                            .unwrap_or(-1.0);
                                        new_folder.sort_index = Some(max_sort_index + 1.0);
                                    }
                                    folders_vec.push(new_folder);
                                });
                                set_folder_name.set(String::from("New Folder"));
                                set_folder_color.set(String::from("#808080"));
                                set_show_folder_dialog.set(false);
                            }
                        }
                        prop:disabled=move || folder_name.get().trim().is_empty()
                    >
                        "Create"
                    </button>
                </div>
            </div>
        </Window>

        <DeleteFolderConfirmation
            folder_delete_pending=folder_delete_pending
            set_folder_delete_pending=set_folder_delete_pending
            folders=folders
            set_folders=set_folders
            set_lines=set_lines
        />

        <EditFolderDialog
            folder_edit_pending=folder_edit_pending
            set_folder_edit_pending=set_folder_edit_pending
            folders=folders
            set_folders=set_folders
        />
    }
}
