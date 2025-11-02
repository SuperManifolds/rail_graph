/// Migration from project version 1 (UUID-based IDs) to version 2 (u64-based IDs)
///
/// This migration converts all UUID fields to u64 using a deterministic hash function.
/// This ensures that the same UUID always maps to the same u64, which is important
/// for preserving relationships and stability across re-migrations.
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// V1 to V2 migration
pub struct V1ToV2Migration;

impl super::Migration for V1ToV2Migration {
    fn source_version(&self) -> u32 {
        1
    }

    fn target_version(&self) -> u32 {
        2
    }

    fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        // Deserialize v1 project
        let v1_project: V1Project = rmp_serde::from_slice(data)
            .map_err(|e| format!("Failed to deserialize v1 project: {e}"))?;

        // Convert to v2
        let v2_project = convert_v1_to_v2(v1_project);

        // Serialize v2 project
        rmp_serde::to_vec(&v2_project)
            .map_err(|e| format!("Failed to serialize v2 project: {e}"))
    }
}

/// Convert UUID to u64 using deterministic hashing
fn uuid_to_u64(uuid: &uuid::Uuid) -> u64 {
    let uuid_bytes = uuid.as_bytes();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    uuid_bytes.hash(&mut hasher);
    hasher.finish()
}

/// Convert optional UUID to optional u64
fn optional_uuid_to_u64(uuid: Option<&uuid::Uuid>) -> Option<u64> {
    uuid.map(uuid_to_u64)
}

/// Convert v1 project to v2
fn convert_v1_to_v2(v1: V1Project) -> crate::models::Project {
    let metadata = crate::models::ProjectMetadata {
        id: v1.metadata.id,
        name: v1.metadata.name,
        created_at: v1.metadata.created_at,
        updated_at: v1.metadata.updated_at,
    };

    // Convert lines
    let lines: Vec<_> = v1.lines.into_iter()
        .map(convert_v1_line_to_v2)
        .collect();

    // Convert folders
    let folders: Vec<_> = v1.folders.into_iter()
        .map(convert_v1_folder_to_v2)
        .collect();

    // Convert views
    let views: Vec<_> = v1.views.into_iter()
        .map(convert_v1_view_to_v2)
        .collect();

    crate::models::Project {
        metadata,
        lines,
        graph: v1.graph,
        legend: v1.legend,
        settings: v1.settings,
        views,
        active_tab_id: v1.active_tab_id,
        infrastructure_viewport: v1.infrastructure_viewport,
        folders,
    }
}

fn convert_v1_line_to_v2(v1: V1Line) -> crate::models::Line {
    let departures: Vec<_> = v1.manual_departures.into_iter()
        .map(|dep| {
            crate::models::ManualDeparture {
                id: uuid_to_u64(&dep.id),
                time: dep.time,
                from_station: dep.from_station,
                to_station: dep.to_station,
                days_of_week: dep.days_of_week,
                train_number: dep.train_number,
                repeat_interval: dep.repeat_interval,
                repeat_until: dep.repeat_until,
            }
        })
        .collect();

    crate::models::Line {
        id: uuid_to_u64(&v1.id),
        name: v1.name,
        frequency: v1.frequency,
        color: v1.color,
        thickness: v1.thickness,
        first_departure: v1.first_departure,
        return_first_departure: v1.return_first_departure,
        visible: v1.visible,
        schedule_mode: v1.schedule_mode,
        days_of_week: v1.days_of_week,
        manual_departures: departures,
        forward_route: v1.forward_route,
        return_route: v1.return_route,
        sync_routes: v1.sync_routes,
        auto_train_number_format: v1.auto_train_number_format,
        last_departure: v1.last_departure,
        return_last_departure: v1.return_last_departure,
        default_wait_time: v1.default_wait_time,
        first_stop_wait_time: v1.first_stop_wait_time,
        return_first_stop_wait_time: v1.return_first_stop_wait_time,
        sort_index: v1.sort_index,
        sync_departure_offsets: v1.sync_departure_offsets,
        folder_id: optional_uuid_to_u64(v1.folder_id.as_ref()),
    }
}

fn convert_v1_folder_to_v2(v1: V1LineFolder) -> crate::models::LineFolder {
    crate::models::LineFolder {
        id: uuid_to_u64(&v1.id),
        name: v1.name,
        color: v1.color,
        icon: v1.icon,
        sort_index: v1.sort_index,
        collapsed: v1.collapsed,
        parent_folder_id: optional_uuid_to_u64(v1.parent_folder_id.as_ref()),
    }
}

fn convert_v1_view_to_v2(v1: V1GraphView) -> crate::models::GraphView {
    crate::models::GraphView {
        id: uuid_to_u64(&v1.id),
        name: v1.name,
        viewport_state: v1.viewport_state,
        station_range: v1.station_range,
        edge_path: v1.edge_path,
        source_line_id: optional_uuid_to_u64(v1.source_line_id.as_ref()),
    }
}

// V1 data structures (ProjectMetadata.id is String, all other IDs are uuid::Uuid)

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V1ProjectMetadata {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V1Project {
    #[serde(flatten)]
    pub metadata: V1ProjectMetadata,
    pub lines: Vec<V1Line>,
    pub graph: crate::models::RailwayGraph,
    #[serde(default)]
    pub legend: crate::models::Legend,
    #[serde(default)]
    pub settings: crate::models::ProjectSettings,
    #[serde(default)]
    pub views: Vec<V1GraphView>,
    #[serde(default)]
    pub active_tab_id: Option<String>,
    #[serde(default)]
    pub infrastructure_viewport: crate::models::ViewportState,
    #[serde(default)]
    pub folders: Vec<V1LineFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V1Line {
    pub id: uuid::Uuid,
    pub name: String,
    pub frequency: chrono::Duration,
    pub color: String,
    pub thickness: f64,
    pub first_departure: chrono::NaiveDateTime,
    pub return_first_departure: chrono::NaiveDateTime,
    pub visible: bool,
    pub schedule_mode: crate::models::ScheduleMode,
    pub days_of_week: crate::models::DaysOfWeek,
    pub manual_departures: Vec<V1ManualDeparture>,
    pub forward_route: Vec<crate::models::RouteSegment>,
    pub return_route: Vec<crate::models::RouteSegment>,
    pub sync_routes: bool,
    pub auto_train_number_format: String,
    pub last_departure: chrono::NaiveDateTime,
    pub return_last_departure: chrono::NaiveDateTime,
    pub default_wait_time: chrono::Duration,
    pub first_stop_wait_time: chrono::Duration,
    pub return_first_stop_wait_time: chrono::Duration,
    pub sort_index: Option<f64>,
    pub sync_departure_offsets: bool,
    pub folder_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V1ManualDeparture {
    pub id: uuid::Uuid,
    pub time: chrono::NaiveDateTime,
    pub from_station: petgraph::stable_graph::NodeIndex,
    pub to_station: petgraph::stable_graph::NodeIndex,
    pub days_of_week: crate::models::DaysOfWeek,
    pub train_number: Option<String>,
    pub repeat_interval: Option<chrono::Duration>,
    pub repeat_until: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V1LineFolder {
    pub id: uuid::Uuid,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub sort_index: Option<f64>,
    pub collapsed: bool,
    pub parent_folder_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V1GraphView {
    pub id: uuid::Uuid,
    pub name: String,
    #[serde(default)]
    pub viewport_state: crate::models::ViewportState,
    pub station_range: Option<(petgraph::stable_graph::NodeIndex, petgraph::stable_graph::NodeIndex)>,
    #[serde(default)]
    pub edge_path: Option<Vec<usize>>,
    #[serde(default)]
    pub source_line_id: Option<uuid::Uuid>,
}
