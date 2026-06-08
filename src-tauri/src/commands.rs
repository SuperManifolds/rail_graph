use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use railgraph_core::auto_layout;
use railgraph_core::conflict::{detect_line_conflicts, SerializableConflictContext};
use railgraph_core::models::{Project, ProjectMetadata};
use railgraph_core::train_journey::TrainJourney;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::Manager;

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

// --- Project State Cache (for multi-window sync) ---

/// Cache the current project state in memory so secondary windows can read it on startup.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn cache_project_state(
    state: tauri::State<'_, crate::AppState>,
    request: Request<'_>,
) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected raw binary body".into());
    };
    let mut cache = state
        .project_cache
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    *cache = Some(bytes.clone());
    Ok(())
}

/// Get the cached project state. Returns None if no state has been cached yet.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_cached_project_state(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Response, String> {
    let cache = state
        .project_cache
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    match cache.as_ref() {
        Some(bytes) => Ok(Response::new(bytes.clone())),
        None => Err("No cached project state".into()),
    }
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
