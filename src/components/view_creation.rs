use leptos::{create_signal, create_effect, ReadSignal, WriteSignal, SignalGet, SignalSet, Callable};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use std::rc::Rc;
use crate::models::{RailwayGraph, GraphView, Routes};
use super::infrastructure_toolbar::EditMode;

/// Validate waypoints and update path preview and error state
fn validate_waypoints(
    waypoints: &[NodeIndex],
    graph: &ReadSignal<RailwayGraph>,
    set_preview_path: WriteSignal<Option<Vec<EdgeIndex>>>,
    set_validation_error: WriteSignal<Option<String>>,
) {
    if waypoints.len() >= 2 {
        let current_graph = graph.get();
        if let Some(path) = current_graph.find_multi_point_path(waypoints) {
            set_preview_path.set(Some(path));
            set_validation_error.set(None);
        } else {
            set_preview_path.set(None);
            set_validation_error.set(Some("No valid path exists through these waypoints".to_string()));
        }
    } else {
        set_preview_path.set(None);
        set_validation_error.set(None);
    }
}

pub struct ViewCreationState {
    pub waypoints: ReadSignal<Vec<NodeIndex>>,
    pub set_waypoints: WriteSignal<Vec<NodeIndex>>,
    pub show_dialog: ReadSignal<bool>,
    pub set_show_dialog: WriteSignal<bool>,
    pub validation_error: ReadSignal<Option<String>>,
    pub set_validation_error: WriteSignal<Option<String>>,
    pub preview_path: ReadSignal<Option<Vec<EdgeIndex>>>,
    pub set_preview_path: WriteSignal<Option<Vec<EdgeIndex>>>,
}

impl ViewCreationState {
    #[must_use]
    pub fn new(
        edit_mode: ReadSignal<EditMode>,
    ) -> Self {
        let (waypoints, set_waypoints) = create_signal(Vec::<NodeIndex>::new());
        let (show_dialog, set_show_dialog) = create_signal(false);
        let (validation_error, set_validation_error) = create_signal(None::<String>);
        let (preview_path, set_preview_path) = create_signal(None::<Vec<EdgeIndex>>);

        // Watch for when edit mode changes to CreatingView - open dialog immediately
        create_effect(move |prev_mode: Option<EditMode>| {
            let current_mode = edit_mode.get();
            if matches!(current_mode, EditMode::CreatingView) && prev_mode != Some(EditMode::CreatingView) {
                // Entering CreatingView mode - open dialog and clear state
                set_waypoints.set(Vec::new());
                set_validation_error.set(None);
                set_preview_path.set(None);
                set_show_dialog.set(true);
            } else if !matches!(current_mode, EditMode::CreatingView) && matches!(prev_mode, Some(EditMode::CreatingView)) {
                // Exiting CreatingView mode - close dialog and clear state
                set_show_dialog.set(false);
                set_waypoints.set(Vec::new());
                set_validation_error.set(None);
                set_preview_path.set(None);
            }
            current_mode
        });

        Self {
            waypoints,
            set_waypoints,
            show_dialog,
            set_show_dialog,
            validation_error,
            set_validation_error,
            preview_path,
            set_preview_path,
        }
    }

    #[must_use]
    pub fn create_callbacks(
        &self,
        graph: ReadSignal<RailwayGraph>,
        set_edit_mode: WriteSignal<EditMode>,
        on_create_view: leptos::Callback<GraphView>,
    ) -> ViewCreationCallbacks {
        let waypoints = self.waypoints;
        let set_waypoints = self.set_waypoints;
        let set_show_dialog = self.set_show_dialog;
        let set_validation_error = self.set_validation_error;
        let set_preview_path = self.set_preview_path;

        // Callback for creating a view from waypoints
        let handle_create_view = Rc::new(move |name: String, wps: Vec<NodeIndex>| {
            let current_graph = graph.get();
            match GraphView::from_waypoints(name, &wps, &current_graph) {
                Ok(new_view) => {
                    on_create_view.call(new_view);
                    set_show_dialog.set(false);
                    set_edit_mode.set(EditMode::None);
                    set_waypoints.set(Vec::new());
                    set_validation_error.set(None);
                    set_preview_path.set(None);
                }
                Err(err) => {
                    web_sys::console::error_1(&format!("Failed to create view: {err}").into());
                }
            }
        });

        // Callback for adding a waypoint (from dropdown or canvas click)
        let handle_add_waypoint = Rc::new(move |node_idx: NodeIndex| {
            let mut current_waypoints = waypoints.get();
            current_waypoints.push(node_idx);

            validate_waypoints(&current_waypoints, &graph, set_preview_path, set_validation_error);
            set_waypoints.set(current_waypoints);
        });

        // Callback for removing a waypoint at specific index
        let handle_remove_waypoint = Rc::new(move |index: usize| {
            let mut current_waypoints = waypoints.get();
            if index < current_waypoints.len() {
                current_waypoints.remove(index);
                validate_waypoints(&current_waypoints, &graph, set_preview_path, set_validation_error);
                set_waypoints.set(current_waypoints);
            }
        });

        // Callback for closing dialog
        let handle_close = Rc::new(move || {
            set_show_dialog.set(false);
            set_edit_mode.set(EditMode::None);
        });

        ViewCreationCallbacks {
            on_create: handle_create_view,
            on_add_waypoint: handle_add_waypoint,
            on_remove_waypoint: handle_remove_waypoint,
            on_close: handle_close,
        }
    }
}

pub struct ViewCreationCallbacks {
    pub on_create: Rc<dyn Fn(String, Vec<NodeIndex>)>,
    pub on_add_waypoint: Rc<dyn Fn(NodeIndex)>,
    pub on_remove_waypoint: Rc<dyn Fn(usize)>,
    pub on_close: Rc<dyn Fn()>,
}
