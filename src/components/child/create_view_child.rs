use crate::components::create_view_dialog::CreateViewDialogContent;
use crate::models::{RailwayGraph, Routes};
use crate::window_protocol::{CreateViewInit, CreateViewResult, CreateViewUpdate};
use leptos::{component, create_signal, view, IntoView, SignalGet, SignalGetUntracked, SignalSet};
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;

fn validate_waypoints(
    waypoints: &[NodeIndex],
    graph: &RailwayGraph,
    set_validation_error: leptos::WriteSignal<Option<String>>,
) {
    if waypoints.len() >= 2 {
        if graph.find_multi_point_path(waypoints).is_some() {
            set_validation_error.set(None);
        } else {
            set_validation_error.set(Some(
                "No valid path exists through these waypoints".to_string(),
            ));
        }
    } else {
        set_validation_error.set(None);
    }
}

#[component]
#[must_use]
pub fn CreateViewChild(init: CreateViewInit, session: String) -> impl IntoView {
    let graph_snapshot = init.graph;
    let (graph, _) = create_signal(graph_snapshot.clone());
    let (waypoints, set_waypoints) = create_signal(Vec::<NodeIndex>::new());
    let (validation_error, set_validation_error) = create_signal(None::<String>);

    // Listen for update-data events from the main window (map clicks adding waypoints)
    {
        let graph_for_update = graph_snapshot.clone();
        let session_for_listen = session.clone();
        leptos::spawn_local(async move {
            let event_name = format!("update-data:{session_for_listen}");
            match crate::tauri_bridge::listen_event(&event_name, move |payload| {
                if let Ok(update) = serde_json::from_str::<CreateViewUpdate>(&payload) {
                    let new_waypoints: Vec<NodeIndex> =
                        update.waypoints.into_iter().map(NodeIndex::new).collect();
                    validate_waypoints(&new_waypoints, &graph_for_update, set_validation_error);
                    set_waypoints.set(new_waypoints);
                }
            })
            .await
            {
                Ok(unlisten) => {
                    // Store unlisten handle to prevent GC; window close cleans up
                    std::mem::forget(unlisten);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to listen for update-data: {}", e);
                }
            }
        });
    }

    let session_create = session.clone();
    let on_create = Rc::new(move |name: String, wps: Vec<NodeIndex>| {
        let result = CreateViewResult::Create {
            name,
            waypoints: wps.iter().map(|n| n.index()).collect(),
        };
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_create}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    });

    let on_close = Rc::new(move || {
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::close_native_window("child-create-view").await;
        });
    });

    let graph_for_validate = graph_snapshot;
    let on_add_waypoint = Rc::new(move |node_idx: NodeIndex| {
        let mut current = waypoints.get();
        current.push(node_idx);
        validate_waypoints(&current, &graph_for_validate, set_validation_error);
        set_waypoints.set(current);
    });

    let graph_for_remove = graph.get_untracked();
    let on_remove_waypoint = Rc::new(move |index: usize| {
        let mut current = waypoints.get();
        if index < current.len() {
            current.remove(index);
            validate_waypoints(&current, &graph_for_remove, set_validation_error);
            set_waypoints.set(current);
        }
    });

    view! {
        <CreateViewDialogContent
            waypoints=waypoints
            graph=graph
            validation_error=validation_error
            on_close=on_close
            on_create=on_create
            on_add_waypoint=on_add_waypoint
            on_remove_waypoint=on_remove_waypoint
        />
    }
}
