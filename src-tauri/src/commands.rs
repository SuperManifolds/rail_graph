use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use railgraph_core::auto_layout;
use railgraph_core::conflict::{detect_line_conflicts, SerializableConflictContext};
use railgraph_core::models::{Project, ProjectMetadata};
use railgraph_core::train_journey::TrainJourney;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Emitter, Manager};

/// Get the projects directory, creating it if needed.
fn projects_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    let dir = data_dir.join("projects");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create projects dir: {e}"))?;
    Ok(dir)
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data dir: {e}"))?;
    Ok(data_dir.join("config.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct AppConfig {
    current_project_id: Option<String>,
}

fn load_config(app: &tauri::AppHandle) -> AppConfig {
    let Ok(path) = config_path(app) else {
        return AppConfig::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write config: {e}"))
}

// --- Storage Commands ---

/// Save project bytes to filesystem.
/// Expects raw msgpack bytes in the request body with project ID in the `id` header.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn save_project(app: tauri::AppHandle, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected raw binary body".into());
    };
    let id = request
        .headers()
        .get("id")
        .and_then(|v| v.to_str().ok())
        .ok_or("Missing project ID header")?;

    let dir = projects_dir(&app)?;
    let path = dir.join(format!("{id}.rgproject"));
    std::fs::write(path, bytes).map_err(|e| format!("Failed to write project: {e}"))
}

/// Load project bytes from filesystem. Returns raw msgpack bytes.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn load_project(app: tauri::AppHandle, id: String) -> Result<Response, String> {
    let dir = projects_dir(&app)?;
    let path = dir.join(format!("{id}.rgproject"));
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read project: {e}"))?;
    Ok(Response::new(bytes))
}

/// Delete a project from filesystem.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn delete_project(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let dir = projects_dir(&app)?;
    let path = dir.join(format!("{id}.rgproject"));
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Failed to delete project: {e}"))?;
    }
    Ok(())
}

/// List all saved projects.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_projects(app: tauri::AppHandle) -> Result<Vec<ProjectMetadata>, String> {
    let dir = projects_dir(&app)?;
    let mut projects = Vec::new();

    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("Failed to read projects dir: {e}"))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rgproject") {
            continue;
        }
        let Some(metadata) = read_project_metadata(&path) else {
            continue;
        };
        projects.push(metadata);
    }

    Ok(projects)
}

fn read_project_metadata(path: &std::path::Path) -> Option<ProjectMetadata> {
    let bytes = std::fs::read(path).ok()?;
    let project = Project::from_bytes(&bytes).ok()?;
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let updated_at = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.to_rfc3339()
        })
        .unwrap_or_default();

    Some(ProjectMetadata {
        id,
        name: project.metadata.name,
        created_at: updated_at.clone(),
        updated_at,
    })
}

/// Get the current project ID.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_current_project_id(app: tauri::AppHandle) -> Option<String> {
    load_config(&app).current_project_id
}

/// Set the current project ID.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_current_project_id(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut config = load_config(&app);
    config.current_project_id = Some(id);
    save_config(&app, &config)
}

// --- Helpers exposed for main.rs startup ---

pub fn projects_dir_internal(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    projects_dir(app)
}

pub fn get_current_project_id_internal(app: &tauri::AppHandle) -> Option<String> {
    load_config(app).current_project_id
}

// --- Compute Commands ---

/// Detect conflicts in the project.
/// Expects raw msgpack project bytes in request body.
/// Parameters passed as headers.
#[tauri::command]
pub async fn detect_conflicts(request: Request<'_>) -> Result<Response, String> {
    let InvokeBody::Raw(project_bytes) = request.body() else {
        return Err("Expected raw binary body".into());
    };
    let project_bytes = project_bytes.clone();

    let headers = request.headers();
    let visible_line_ids: Vec<uuid::Uuid> = headers
        .get("visible-line-ids")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let station_margin_ms: i64 = headers
        .get("station-margin-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(30_000);

    let minimum_separation_ms: i64 = headers
        .get("minimum-separation-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(30_000);

    let ignore_same_dir: bool = headers
        .get("ignore-same-dir")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s == "true");

    let day_filter: Option<chrono::Weekday> = headers
        .get("day-filter")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    let view_edge_filter: Option<Vec<usize>> = headers
        .get("view-edge-filter")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok());

    let result = tauri::async_runtime::spawn_blocking(move || {
        compute_conflicts(
            &project_bytes,
            &visible_line_ids,
            station_margin_ms,
            minimum_separation_ms,
            ignore_same_dir,
            day_filter,
            view_edge_filter.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    Ok(Response::new(result))
}

fn compute_conflicts(
    project_bytes: &[u8],
    visible_line_ids: &[uuid::Uuid],
    station_margin_ms: i64,
    minimum_separation_ms: i64,
    ignore_same_dir: bool,
    day_filter: Option<chrono::Weekday>,
    view_edge_filter: Option<&[usize]>,
) -> Result<Vec<u8>, String> {
    let project = Project::from_bytes(project_bytes)
        .map_err(|e| format!("Failed to deserialize project: {e}"))?;

    let visible_set: HashSet<_> = visible_line_ids.iter().collect();

    let visible_lines: Vec<_> = if let Some(view_edges) = view_edge_filter {
        filter_lines_by_view(&project, &visible_set, view_edges)
    } else {
        project
            .lines
            .iter()
            .filter(|line| visible_set.contains(&line.id))
            .cloned()
            .collect()
    };

    let journeys = TrainJourney::generate_journeys(&visible_lines, &project.graph, day_filter);
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    let station_indices = project
        .graph
        .graph
        .node_indices()
        .enumerate()
        .map(|(idx, node_idx)| (node_idx, idx))
        .collect();

    let context = SerializableConflictContext::from_graph(
        &project.graph,
        station_indices,
        chrono::Duration::milliseconds(station_margin_ms),
        chrono::Duration::milliseconds(minimum_separation_ms),
        ignore_same_dir,
    );

    let (conflicts, _) = detect_line_conflicts(&journeys_vec, &context);

    rmp_serde::to_vec(&conflicts).map_err(|e| format!("Failed to serialize conflicts: {e}"))
}

fn filter_lines_by_view(
    project: &Project,
    visible_set: &HashSet<&uuid::Uuid>,
    view_edges: &[usize],
) -> Vec<railgraph_core::models::Line> {
    let view_edge_set: HashSet<usize> = view_edges.iter().copied().collect();
    let mut view_station_set: HashSet<petgraph::stable_graph::NodeIndex> = HashSet::new();
    for &edge_idx in view_edges {
        let edge_index = petgraph::stable_graph::EdgeIndex::new(edge_idx);
        if let Some((a, b)) = project.graph.graph.edge_endpoints(edge_index) {
            view_station_set.insert(a);
            view_station_set.insert(b);
        }
    }

    project
        .lines
        .iter()
        .filter(|line| visible_set.contains(&line.id))
        .filter(|line| {
            line_touches_view(line, &project.graph, &view_edge_set, &view_station_set)
        })
        .cloned()
        .collect()
}

fn line_touches_view(
    line: &railgraph_core::models::Line,
    graph: &railgraph_core::models::RailwayGraph,
    view_edge_set: &HashSet<usize>,
    view_station_set: &HashSet<petgraph::stable_graph::NodeIndex>,
) -> bool {
    line.forward_route
        .iter()
        .chain(line.return_route.iter())
        .any(|seg| {
            if view_edge_set.contains(&seg.edge_index) {
                return true;
            }
            let edge_index = petgraph::stable_graph::EdgeIndex::new(seg.edge_index);
            project_graph_touches_view(graph, edge_index, view_station_set)
        })
}

fn project_graph_touches_view(
    graph: &railgraph_core::models::RailwayGraph,
    edge_index: petgraph::stable_graph::EdgeIndex,
    view_station_set: &HashSet<petgraph::stable_graph::NodeIndex>,
) -> bool {
    if let Some((a, b)) = graph.graph.edge_endpoints(edge_index) {
        return view_station_set.contains(&a) || view_station_set.contains(&b);
    }
    false
}

/// Compute auto-layout positions for the graph.
/// Expects raw msgpack bytes of `LayoutRequest`.
/// Returns raw msgpack `HashMap<usize, (f64, f64)>` of node positions.
#[tauri::command]
pub async fn compute_auto_layout(request: Request<'_>) -> Result<Response, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected raw binary body".into());
    };
    let bytes = bytes.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        #[derive(serde::Deserialize)]
        struct LayoutRequest {
            graph: railgraph_core::models::RailwayGraph,
            geo_hints: Option<auto_layout::GeographicHints>,
            settings: railgraph_core::models::ProjectSettings,
            height: f64,
        }

        let req: LayoutRequest = rmp_serde::from_slice(&bytes)
            .map_err(|e| format!("Failed to deserialize layout request: {e}"))?;

        let mut graph = req.graph;
        auto_layout::apply_layout(&mut graph, req.height, &req.settings, req.geo_hints.as_ref());

        let positions: HashMap<usize, (f64, f64)> = graph
            .graph
            .node_indices()
            .filter_map(|idx| {
                let (x, y) = graph.graph.node_weight(idx)?.position()?;
                Some((idx.index(), (x, y)))
            })
            .collect();

        rmp_serde::to_vec(&positions)
            .map_err(|e| format!("Failed to serialize positions: {e}"))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    Ok(Response::new(result))
}

// --- Backend-Managed Project State ---

use base64::Engine;
use railgraph_core::models::UndoSnapshot;

fn emit_field(
    app: &tauri::AppHandle,
    field: &str,
    data: &[u8],
    source: &str,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "field": field,
        "data": base64::engine::general_purpose::STANDARD.encode(data),
        "source": source,
    });
    app.emit("field-updated", payload)
        .map_err(|e| format!("Emit failed: {e}"))
}

fn queue_debounced_save(app: &tauri::AppHandle, state: &crate::AppState) {
    let bytes = {
        let project = state.project.lock().expect("project lock");
        match project.serialize_to_bytes() {
            Ok(b) => b,
            Err(e) => {
                log::error!("Failed to serialize project for save: {e}");
                return;
            }
        }
    };

    let id = state
        .project
        .lock()
        .expect("project lock")
        .metadata
        .id
        .clone();
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let dir = match projects_dir(&app) {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to get projects dir: {e}");
                return;
            }
        };
        let path = dir.join(format!("{id}.rgproject"));
        if let Err(e) = std::fs::write(&path, &bytes) {
            log::error!("Failed to save project: {e}");
        }
    });
}

/// Update a single field of the canonical project state.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn update_field(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    request: Request<'_>,
) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected raw binary body".into());
    };
    let field = request
        .headers()
        .get("field")
        .and_then(|v| v.to_str().ok())
        .ok_or("Missing field header")?;
    let source = request
        .headers()
        .get("source")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    let mut project = state
        .project
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;

    if field == "graph" || field == "lines" {
        let snapshot = UndoSnapshot::new(project.graph.clone(), project.lines.clone());
        if let Ok(mut mgr) = state.undo_manager.lock() {
            mgr.push_snapshot(snapshot);
        }
    }

    match field {
        "graph" => {
            project.graph = rmp_serde::from_slice(bytes)
                .map_err(|e| format!("Deserialize graph: {e}"))?;
        }
        "lines" => {
            project.lines = rmp_serde::from_slice(bytes)
                .map_err(|e| format!("Deserialize lines: {e}"))?;
        }
        "views" => {
            project.views = rmp_serde::from_slice(bytes)
                .map_err(|e| format!("Deserialize views: {e}"))?;
        }
        "folders" => {
            project.folders = rmp_serde::from_slice(bytes)
                .map_err(|e| format!("Deserialize folders: {e}"))?;
        }
        "settings" => {
            project.settings = rmp_serde::from_slice(bytes)
                .map_err(|e| format!("Deserialize settings: {e}"))?;
        }
        "legend" => {
            project.legend = rmp_serde::from_slice(bytes)
                .map_err(|e| format!("Deserialize legend: {e}"))?;
        }
        _ => return Err(format!("Unknown field: {field}")),
    }

    project.touch_updated_at();
    drop(project);

    emit_field(&app, field, bytes, source)?;
    queue_debounced_save(&app, &state);
    Ok(())
}

/// Return the full serialized project state for window initialization.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn load_project_state(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Response, String> {
    let project = state
        .project
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    let bytes = project
        .serialize_to_bytes()
        .map_err(|e| format!("Serialize: {e}"))?;
    Ok(Response::new(bytes))
}

/// Replace the entire project (for project load/import).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn replace_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    request: Request<'_>,
) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected raw binary body".into());
    };
    let new_project = Project::from_bytes(bytes)
        .map_err(|e| format!("Deserialize: {e}"))?;

    let id = new_project.metadata.id.clone();

    let mut project = state
        .project
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    *project = new_project;
    drop(project);

    if let Ok(mut mgr) = state.undo_manager.lock() {
        mgr.clear();
    }

    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let payload = serde_json::json!({ "data": encoded });
    app.emit("project-replaced", payload)
        .map_err(|e| format!("Emit: {e}"))?;

    let mut config = load_config(&app);
    config.current_project_id = Some(id);
    save_config(&app, &config)?;
    queue_debounced_save(&app, &state);
    Ok(())
}

/// Undo the last graph/lines change.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn undo(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<bool, String> {
    let mut project = state
        .project
        .lock()
        .map_err(|e| format!("Lock: {e}"))?;
    let mut mgr = state
        .undo_manager
        .lock()
        .map_err(|e| format!("Lock: {e}"))?;

    let current = UndoSnapshot::new(project.graph.clone(), project.lines.clone());
    let Some(snapshot) = mgr.undo(current) else {
        return Ok(false);
    };

    project.graph = snapshot.graph;
    project.lines = snapshot.lines;
    project.touch_updated_at();

    let graph_bytes = rmp_serde::to_vec(&project.graph)
        .map_err(|e| format!("Serialize graph: {e}"))?;
    let lines_bytes = rmp_serde::to_vec(&project.lines)
        .map_err(|e| format!("Serialize lines: {e}"))?;
    drop(project);
    drop(mgr);

    emit_field(&app, "graph", &graph_bytes, "backend")?;
    emit_field(&app, "lines", &lines_bytes, "backend")?;
    queue_debounced_save(&app, &state);
    Ok(true)
}

/// Redo the last undone graph/lines change.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn redo(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<bool, String> {
    let mut project = state
        .project
        .lock()
        .map_err(|e| format!("Lock: {e}"))?;
    let mut mgr = state
        .undo_manager
        .lock()
        .map_err(|e| format!("Lock: {e}"))?;

    let current = UndoSnapshot::new(project.graph.clone(), project.lines.clone());
    let Some(snapshot) = mgr.redo(current) else {
        return Ok(false);
    };

    project.graph = snapshot.graph;
    project.lines = snapshot.lines;
    project.touch_updated_at();

    let graph_bytes = rmp_serde::to_vec(&project.graph)
        .map_err(|e| format!("Serialize graph: {e}"))?;
    let lines_bytes = rmp_serde::to_vec(&project.lines)
        .map_err(|e| format!("Serialize lines: {e}"))?;
    drop(project);
    drop(mgr);

    emit_field(&app, "graph", &graph_bytes, "backend")?;
    emit_field(&app, "lines", &lines_bytes, "backend")?;
    queue_debounced_save(&app, &state);
    Ok(true)
}

/// Save per-window metadata (viewport states, window layouts) into the project.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn save_window_metadata(
    state: tauri::State<'_, crate::AppState>,
    window_layouts: String,
    viewport_states: String,
) -> Result<(), String> {
    let layouts: Vec<railgraph_core::models::WindowLayout> =
        serde_json::from_str(&window_layouts)
            .map_err(|e| format!("Deserialize layouts: {e}"))?;
    let viewports: HashMap<String, railgraph_core::models::ViewportState> =
        serde_json::from_str(&viewport_states)
            .map_err(|e| format!("Deserialize viewports: {e}"))?;

    let mut project = state
        .project
        .lock()
        .map_err(|e| format!("Lock: {e}"))?;

    project.window_layouts = layouts;

    for view in &mut project.views {
        if let Some(vp) = viewports.get(&view.id.to_string()) {
            view.viewport_state = vp.clone();
        }
    }

    Ok(())
}
