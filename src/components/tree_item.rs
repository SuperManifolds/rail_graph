use leptos::{component, view, ReadSignal, WriteSignal, IntoView, create_memo, Signal, SignalGet, SignalGetUntracked, SignalUpdate, SignalSet, SignalWith, For, store_value, Callback, Callable};
use crate::models::{Line, LineFolder, RailwayGraph, GraphView, ViewportState, LineSortMode, Routes};
use crate::components::dropdown_menu::{DropdownMenu, MenuItem};
use crate::components::line_controls::{handle_drop_into_folder, handle_drop_in_zone};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum TreeItem {
    Folder {
        folder: LineFolder,
        children: Vec<TreeItem>,
    },
    Line(Line),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DraggedItem {
    Line(uuid::Uuid),
    Folder(uuid::Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropZone {
    Before(uuid::Uuid),
    After(uuid::Uuid),
}

impl TreeItem {
    #[must_use]
    pub fn id(&self) -> uuid::Uuid {
        match self {
            TreeItem::Folder { folder, .. } => folder.id,
            TreeItem::Line(line) => line.id,
        }
    }

    #[must_use]
    pub fn sort_index(&self) -> Option<f64> {
        match self {
            TreeItem::Folder { folder, .. } => folder.sort_index,
            TreeItem::Line(line) => line.sort_index,
        }
    }
}

#[must_use]
pub fn build_tree(lines: Vec<Line>, folders: Vec<LineFolder>, sort_mode: LineSortMode) -> Vec<TreeItem> {
    // Build a set of valid folder IDs
    let valid_folder_ids: std::collections::HashSet<uuid::Uuid> =
        folders.iter().map(|f| f.id).collect();

    // Build a map of folder_id -> children
    let mut folder_children: HashMap<Option<uuid::Uuid>, Vec<TreeItem>> = HashMap::new();

    // Add all lines as tree items
    for line in lines {
        let tree_item = TreeItem::Line(line.clone());
        // If line references an invalid folder, treat it as a root line
        let effective_folder_id = match line.folder_id {
            Some(id) if valid_folder_ids.contains(&id) => Some(id),
            _ => None,
        };
        folder_children.entry(effective_folder_id)
            .or_default()
            .push(tree_item);
    }

    // Group folders by parent, checking for cycles
    let mut folder_map: HashMap<Option<uuid::Uuid>, Vec<LineFolder>> = HashMap::new();
    let folders_vec = folders.clone();

    for folder in folders {
        // Check if this folder's parent chain is valid (no cycles, no invalid refs)
        let effective_parent_id = match folder.parent_folder_id {
            Some(parent_id) if valid_folder_ids.contains(&parent_id) => {
                // Check if parent chain leads to a cycle
                let mut current_id = Some(parent_id);
                let mut visited = std::collections::HashSet::new();
                let mut has_cycle = false;

                while let Some(id) = current_id {
                    if id == folder.id {
                        // Found a cycle back to this folder
                        has_cycle = true;
                        break;
                    }
                    if !visited.insert(id) {
                        // Found a cycle (but not to this folder)
                        has_cycle = true;
                        break;
                    }
                    current_id = folders_vec.iter()
                        .find(|f| f.id == id)
                        .and_then(|f| f.parent_folder_id);
                }

                if has_cycle {
                    None
                } else {
                    Some(parent_id)
                }
            }
            _ => None,
        };

        folder_map.entry(effective_parent_id)
            .or_default()
            .push(folder);
    }

    // Build tree starting from root (parent_folder_id = None)
    let mut root_items = Vec::new();
    let mut orphaned_items = Vec::new();

    // Add root folders
    if let Some(root_folders) = folder_map.get(&None) {
        for folder in root_folders {
            let mut visited = std::collections::HashSet::new();
            let folder_tree = build_folder_tree_recursive(
                folder.clone(),
                &folder_map,
                &folder_children,
                sort_mode,
                &mut visited,
                &mut orphaned_items,
            );
            root_items.push(folder_tree);
        }
    }

    // Add root lines (lines with folder_id = None or invalid folder_id)
    if let Some(root_lines) = folder_children.get(&None) {
        root_items.extend(root_lines.clone());
    }

    // Add orphaned items (from circular folder references)
    root_items.extend(orphaned_items);

    // Sort root items
    sort_items(&mut root_items, sort_mode);
    root_items
}

fn build_folder_tree_recursive(
    folder: LineFolder,
    folder_map: &HashMap<Option<uuid::Uuid>, Vec<LineFolder>>,
    folder_children: &HashMap<Option<uuid::Uuid>, Vec<TreeItem>>,
    sort_mode: LineSortMode,
    visited: &mut std::collections::HashSet<uuid::Uuid>,
    orphaned_items: &mut Vec<TreeItem>,
) -> TreeItem {
    let mut children = Vec::new();

    // Mark this folder as visited to prevent infinite recursion
    visited.insert(folder.id);

    // Add subfolders
    if let Some(subfolders) = folder_map.get(&Some(folder.id)) {
        for subfolder in subfolders {
            // Skip if we've already visited this folder (circular reference)
            if visited.contains(&subfolder.id) {
                // Add the subfolder's contents as orphaned items
                if let Some(orphaned_lines) = folder_children.get(&Some(subfolder.id)) {
                    orphaned_items.extend(orphaned_lines.clone());
                }
                continue;
            }
            let subfolder_tree = build_folder_tree_recursive(
                subfolder.clone(),
                folder_map,
                folder_children,
                sort_mode,
                visited,
                orphaned_items,
            );
            children.push(subfolder_tree);
        }
    }

    // Add lines in this folder
    if let Some(lines) = folder_children.get(&Some(folder.id)) {
        children.extend(lines.clone());
    }

    // Sort children
    sort_items(&mut children, sort_mode);

    TreeItem::Folder { folder, children }
}

fn sort_items(items: &mut [TreeItem], mode: LineSortMode) {
    match mode {
        LineSortMode::AddedOrder => {
            // Keep original order
        }
        LineSortMode::Alphabetical => {
            items.sort_by(|a, b| {
                let a_name = match a {
                    TreeItem::Folder { folder, .. } => &folder.name,
                    TreeItem::Line(line) => &line.name,
                };
                let b_name = match b {
                    TreeItem::Folder { folder, .. } => &folder.name,
                    TreeItem::Line(line) => &line.name,
                };
                a_name.cmp(b_name)
            });
        }
        LineSortMode::Manual => {
            items.sort_by(|a, b| {
                // In manual mode, all items should have sort_index by now
                let a_key = a.sort_index().unwrap_or(f64::MAX);
                let b_key = b.sort_index().unwrap_or(f64::MAX);
                a_key.partial_cmp(&b_key).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}

#[must_use]
pub fn find_folder_children(items: &[TreeItem], target_id: uuid::Uuid) -> Option<Vec<TreeItem>> {
    for item in items {
        match item {
            TreeItem::Folder { folder, children } => {
                if folder.id == target_id {
                    return Some(children.clone());
                }
                if let Some(found) = find_folder_children(children, target_id) {
                    return Some(found);
                }
            }
            TreeItem::Line(_) => {}
        }
    }
    None
}

// Find an item in the tree and return its siblings and parent folder context
#[must_use]
pub fn find_item_context(
    tree_items: &[TreeItem],
    target_id: uuid::Uuid,
    parent_folder_id: Option<uuid::Uuid>,
) -> Option<(Vec<TreeItem>, Option<uuid::Uuid>)> {
    // Check if the item is at this level
    if tree_items.iter().any(|item| item.id() == target_id) {
        return Some((tree_items.to_vec(), parent_folder_id));
    }

    // Recursively search in folder children
    for item in tree_items {
        if let TreeItem::Folder { folder, children } = item {
            if let Some(context) = find_item_context(children, target_id, Some(folder.id)) {
                return Some(context);
            }
        }
    }

    None
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn LineControl(
    line_id: uuid::Uuid,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
    dragged_item: ReadSignal<Option<DraggedItem>>,
    set_dragged_item: WriteSignal<Option<DraggedItem>>,
    set_drag_over_id: WriteSignal<Option<uuid::Uuid>>,
    on_edit: impl Fn(uuid::Uuid) + 'static,
    on_delete: impl Fn(uuid::Uuid) + 'static,
    on_duplicate: impl Fn(uuid::Uuid) + 'static,
    on_create_view: Callback<GraphView>,
    #[prop(default = 0)]
    depth: usize,
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
                let is_dragging = move || dragged_item.get() == Some(DraggedItem::Line(line_id));

                view! {
                    <div
                        class=move || {
                            let mut classes = vec!["line-control"];
                            if is_dragging() { classes.push("dragging"); }
                            classes.join(" ")
                        }
                        style=format!("--line-color: {}; margin-left: {}px", line.color, depth * 16)
                        draggable="true"
                        on:dragstart=move |ev| {
                            if let Some(dt) = ev.data_transfer() {
                                let _ = dt.set_data("text/plain", &format!("line:{line_id}"));
                                dt.set_effect_allowed("move");
                            }
                            set_dragged_item.set(Some(DraggedItem::Line(line_id)));
                            set_settings.update(|s| s.line_sort_mode = LineSortMode::Manual);
                        }
                        on:dragend=move |_| {
                            set_dragged_item.set(None);
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

#[component]
pub fn TreeItem(
    tree_item: TreeItem,
    is_last: bool,
    tree: leptos::Memo<Vec<TreeItem>>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    folders: ReadSignal<Vec<LineFolder>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
    graph: ReadSignal<RailwayGraph>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
    dragged_item: ReadSignal<Option<DraggedItem>>,
    set_dragged_item: WriteSignal<Option<DraggedItem>>,
    drag_over_id: ReadSignal<Option<uuid::Uuid>>,
    set_drag_over_id: WriteSignal<Option<uuid::Uuid>>,
    drop_zone_hover: ReadSignal<Option<DropZone>>,
    set_drop_zone_hover: WriteSignal<Option<DropZone>>,
    on_edit: impl Fn(uuid::Uuid) + 'static + Clone,
    on_delete: impl Fn(uuid::Uuid) + 'static + Clone,
    on_duplicate: impl Fn(uuid::Uuid) + 'static + Clone,
    on_folder_edit: impl Fn(uuid::Uuid) + 'static + Clone,
    on_folder_delete: impl Fn(uuid::Uuid) + 'static + Clone,
    on_create_view: Callback<GraphView>,
    depth: usize,
) -> impl IntoView {
    let item_id = tree_item.id();

    let item_view = match tree_item {
        TreeItem::Line(line) => {
            view! {
                <LineControl
                    line_id=line.id
                    lines=lines
                    set_lines=set_lines
                    graph=graph
                    set_settings=set_settings
                    dragged_item=dragged_item
                    set_dragged_item=set_dragged_item
                    set_drag_over_id=set_drag_over_id
                    on_edit=on_edit.clone()
                    on_delete=on_delete.clone()
                    on_duplicate=on_duplicate.clone()
                    on_create_view=on_create_view
                    depth=depth
                />
            }.into_view()
        }
        TreeItem::Folder { folder, .. } => {
            view! {
                <FolderControl
                    folder_id=folder.id
                    folders=folders
                    set_folders=set_folders
                    lines=lines
                    set_lines=set_lines
                    graph=graph
                    settings=settings
                    set_settings=set_settings
                    dragged_item=dragged_item
                    set_dragged_item=set_dragged_item
                    drag_over_id=drag_over_id
                    set_drag_over_id=set_drag_over_id
                    drop_zone_hover=drop_zone_hover
                    set_drop_zone_hover=set_drop_zone_hover
                    on_edit=on_edit
                    on_delete=on_delete
                    on_duplicate=on_duplicate
                    on_folder_edit=on_folder_edit
                    on_folder_delete=on_folder_delete
                    on_create_view=on_create_view
                    depth=depth
                />
            }.into_view()
        }
    };

    view! {
        <div class="tree-item-wrapper">
            <div
                class=move || {
                    let mut classes = vec!["drop-zone", "drop-zone-before"];
                    if drop_zone_hover.get() == Some(DropZone::Before(item_id)) {
                        classes.push("drop-zone-active");
                    }
                    classes.join(" ")
                }
                on:dragover=move |ev| {
                    ev.prevent_default();
                    if let Some(dt) = ev.data_transfer() {
                        dt.set_drop_effect("move");
                    }
                    set_drop_zone_hover.set(Some(DropZone::Before(item_id)));
                }
                on:dragleave=move |_| {
                    set_drop_zone_hover.set(None);
                }
                on:drop=move |ev| {
                    ev.prevent_default();
                    ev.stop_propagation();

                    if let Some(dragged) = dragged_item.get_untracked() {
                        let tree_items = tree.get_untracked();
                        handle_drop_in_zone(
                            dragged,
                            DropZone::Before(item_id),
                            tree_items,
                            set_lines,
                            set_folders,
                        );
                        set_settings.update(|s| s.line_sort_mode = LineSortMode::Manual);
                    }
                    set_dragged_item.set(None);
                    set_drag_over_id.set(None);
                    set_drop_zone_hover.set(None);
                }
            />
            {item_view}
            {is_last.then(|| view! {
                <div
                    class=move || {
                        let mut classes = vec!["drop-zone", "drop-zone-after"];
                        if drop_zone_hover.get() == Some(DropZone::After(item_id)) {
                            classes.push("drop-zone-active");
                        }
                        classes.join(" ")
                    }
                    on:dragover=move |ev| {
                        ev.prevent_default();
                        if let Some(dt) = ev.data_transfer() {
                            dt.set_drop_effect("move");
                        }
                        set_drop_zone_hover.set(Some(DropZone::After(item_id)));
                    }
                    on:dragleave=move |_| {
                        set_drop_zone_hover.set(None);
                    }
                    on:drop=move |ev| {
                        ev.prevent_default();
                        ev.stop_propagation();

                        if let Some(dragged) = dragged_item.get_untracked() {
                            let tree_items = tree.get_untracked();
                            handle_drop_in_zone(
                                dragged,
                                DropZone::After(item_id),
                                tree_items,
                                set_lines,
                                set_folders,
                            );
                            set_settings.update(|s| s.line_sort_mode = LineSortMode::Manual);
                        }
                        set_dragged_item.set(None);
                        set_drag_over_id.set(None);
                        set_drop_zone_hover.set(None);
                    }
                />
            })}
        </div>
    }
}

#[component]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub fn FolderControl(
    folder_id: uuid::Uuid,
    folders: ReadSignal<Vec<LineFolder>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
    dragged_item: ReadSignal<Option<DraggedItem>>,
    set_dragged_item: WriteSignal<Option<DraggedItem>>,
    drag_over_id: ReadSignal<Option<uuid::Uuid>>,
    set_drag_over_id: WriteSignal<Option<uuid::Uuid>>,
    drop_zone_hover: ReadSignal<Option<DropZone>>,
    set_drop_zone_hover: WriteSignal<Option<DropZone>>,
    on_edit: impl Fn(uuid::Uuid) + 'static + Clone,
    on_delete: impl Fn(uuid::Uuid) + 'static + Clone,
    on_duplicate: impl Fn(uuid::Uuid) + 'static + Clone,
    on_folder_edit: impl Fn(uuid::Uuid) + 'static + Clone,
    on_folder_delete: impl Fn(uuid::Uuid) + 'static + Clone,
    on_create_view: Callback<GraphView>,
    depth: usize,
) -> impl IntoView {
    let current_folder = Signal::derive(move || {
        folders.get().into_iter().find(|f| f.id == folder_id)
    });

    let tree_signal = create_memo(move |_| {
        let lines_vec = lines.get();
        let folders_vec = folders.get();
        let sort_mode = settings.with(|s| s.line_sort_mode);
        build_tree(lines_vec, folders_vec, sort_mode)
    });

    let folder_children = create_memo(move |_| {
        find_folder_children(&tree_signal.get(), folder_id).unwrap_or_default()
    });

    view! {
        {move || {
            current_folder.get().map(|folder| {
                let is_collapsed = folder.collapsed;
                let is_drag_over = move || drag_over_id.get() == Some(folder_id);

                view! {
                    <div class="folder-control">
                        <header
                            class=move || {
                                let mut classes = vec!["folder-header"];
                                if is_drag_over() { classes.push("drag-over"); }
                                classes.join(" ")
                            }
                            style=format!("--folder-color: {}; margin-left: {}px", folder.color, depth * 16)
                            draggable="true"
                            on:dragstart=move |ev| {
                                if let Some(dt) = ev.data_transfer() {
                                    let _ = dt.set_data("text/plain", &format!("folder:{folder_id}"));
                                    dt.set_effect_allowed("move");
                                }
                                set_dragged_item.set(Some(DraggedItem::Folder(folder_id)));
                                set_settings.update(|s| s.line_sort_mode = LineSortMode::Manual);
                            }
                            on:dragover=move |ev| {
                                ev.prevent_default();
                                if let Some(dt) = ev.data_transfer() {
                                    dt.set_drop_effect("move");
                                }
                                set_drag_over_id.set(Some(folder_id));
                            }
                            on:dragleave=move |_| {
                                set_drag_over_id.set(None);
                            }
                            on:drop=move |ev| {
                                ev.prevent_default();
                                ev.stop_propagation();

                                if let Some(dragged) = dragged_item.get_untracked() {
                                    handle_drop_into_folder(dragged, folder_id, set_lines, set_folders);
                                }
                                set_dragged_item.set(None);
                                set_drag_over_id.set(None);
                                set_drop_zone_hover.set(None);
                            }
                            on:dragend=move |_| {
                                set_dragged_item.set(None);
                                set_drag_over_id.set(None);
                            }
                        >
                            <button
                                class="folder-toggle"
                                on:click=move |_| {
                                    set_folders.update(|folders_vec| {
                                        if let Some(f) = folders_vec.iter_mut().find(|f| f.id == folder_id) {
                                            f.collapsed = !f.collapsed;
                                        }
                                    });
                                }
                                title=if is_collapsed { "Expand folder" } else { "Collapse folder" }
                            >
                                <i class=if is_collapsed { "fa-solid fa-chevron-right" } else { "fa-solid fa-chevron-down" }></i>
                            </button>
                            <i class="fa-solid fa-folder"></i>
                            <strong>{folder.name.clone()}</strong>
                            <div class="folder-header-controls">
                                <DropdownMenu items={
                                    let on_folder_edit = on_folder_edit.clone();
                                    let on_folder_delete = on_folder_delete.clone();
                                    vec![
                                        MenuItem {
                                            label: "Edit",
                                            icon: "fa-solid fa-pen",
                                            on_click: Rc::new(move || on_folder_edit(folder_id)),
                                        },
                                        MenuItem {
                                            label: "Delete",
                                            icon: "fa-solid fa-trash",
                                            on_click: Rc::new(move || on_folder_delete(folder_id)),
                                        },
                                    ]
                                } />
                            </div>
                        </header>

                        {if is_collapsed {
                            view! {}.into_view()
                        } else {
                            view! {
                                <div class="folder-children">
                                    <For
                                        each={move || {
                                            let children = folder_children.get();
                                            let len = children.len();
                                            children.into_iter().enumerate().map(|(idx, item)| (item, idx == len - 1)).collect::<Vec<_>>()
                                        }}
                                        key={|(item, _)| item.id()}
                                        children={
                                            let on_edit = on_edit.clone();
                                            let on_delete = on_delete.clone();
                                            let on_duplicate = on_duplicate.clone();
                                            let on_folder_edit = on_folder_edit.clone();
                                            let on_folder_delete = on_folder_delete.clone();
                                            move |(child_item, is_last): (TreeItem, bool)| {
                                                view! {
                                                    <TreeItem
                                                        tree_item=child_item
                                                        is_last=is_last
                                                        tree=tree_signal
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
                                                        on_edit=on_edit.clone()
                                                        on_delete=on_delete.clone()
                                                        on_duplicate=on_duplicate.clone()
                                                        on_folder_edit=on_folder_edit.clone()
                                                        on_folder_delete=on_folder_delete.clone()
                                                        on_create_view=on_create_view
                                                        depth=depth + 1
                                                    />
                                                }
                                            }
                                        }
                                    />
                                </div>
                            }.into_view()
                        }}
                    </div>
                }
            })
        }}
    }
}
