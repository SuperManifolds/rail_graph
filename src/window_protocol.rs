//! Typed init/result structs for native window communication.
//! Each window type has a pair of structs: one for initialization data
//! sent from the main window, and one for results sent back.

use crate::import::csv::CsvImportConfig;
use crate::import::nimby::{NimbyImportConfig, NimbyImportData};
use crate::models::{
    Line, Platform, ProjectSettings, RailwayGraph, RoutingRule, Track, TrackHandedness,
};
use serde::{Deserialize, Serialize};

// --- Edit Track ---

#[derive(Serialize, Deserialize)]
pub struct EditTrackInit {
    pub edge_idx: usize,
    pub tracks: Vec<Track>,
    pub distance: Option<f64>,
    pub from_station_name: String,
    pub to_station_name: String,
    pub affected_lines: Vec<String>,
    pub track_handedness: TrackHandedness,
}

#[derive(Serialize, Deserialize)]
pub enum EditTrackResult {
    Save {
        edge_idx: usize,
        tracks: Vec<Track>,
        distance: Option<f64>,
    },
    Delete {
        edge_idx: usize,
    },
}

// --- Edit Junction ---

#[derive(Serialize, Deserialize)]
pub struct EditJunctionInit {
    pub junction_idx: usize,
    pub junction_name: String,
    pub graph: RailwayGraph,
}

#[derive(Serialize, Deserialize)]
pub enum EditJunctionResult {
    Save {
        junction_idx: usize,
        name: Option<String>,
        routing_rules: Vec<RoutingRule>,
    },
    Delete {
        junction_idx: usize,
    },
}

// --- Edit Station ---

#[derive(Serialize, Deserialize)]
pub struct EditStationInit {
    pub station_idx: usize,
    pub station_name: String,
    pub is_passing_loop: bool,
    pub platforms: Vec<Platform>,
    pub graph: RailwayGraph,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TrackDefaultChange {
    pub edge_idx: usize,
    pub default_platform_source: Option<usize>,
    pub default_platform_target: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub enum EditStationResult {
    Save {
        station_idx: usize,
        name: String,
        is_passing_loop: bool,
        platforms: Vec<Platform>,
        track_defaults: Vec<TrackDefaultChange>,
        new_connections: Vec<usize>,
    },
    Delete {
        station_idx: usize,
    },
}

// --- Line Editor ---

#[derive(Serialize, Deserialize)]
pub struct LineEditorInit {
    pub line: Line,
    pub graph: RailwayGraph,
    pub settings: ProjectSettings,
    pub initial_tab: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum LineEditorResult {
    Save(Line),
}

// --- Settings ---

#[derive(Serialize, Deserialize)]
pub struct SettingsInit {
    pub settings: ProjectSettings,
}

#[derive(Serialize, Deserialize)]
pub struct SettingsResult {
    pub settings: ProjectSettings,
}

// --- CSV Column Mapper ---

#[derive(Serialize, Deserialize)]
pub struct ImporterCsvInit {
    pub config: CsvImportConfig,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum ImporterCsvResult {
    Import(CsvImportConfig),
    Cancel,
}

/// Update pushed from the parent window to the open CSV mapper,
/// currently used to surface import errors raised during execution.
#[derive(Serialize, Deserialize)]
pub struct ImporterCsvUpdate {
    pub error: Option<String>,
}

// --- NIMBY Line Selector ---

#[derive(Serialize, Deserialize)]
pub struct ImporterNimbyInit {
    pub data: NimbyImportData,
    pub handedness: TrackHandedness,
    pub station_spacing: f64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum ImporterNimbyResult {
    Import(NimbyImportConfig),
    Cancel,
}

/// Update pushed from the parent window to the open NIMBY selector,
/// currently used to surface import errors raised during execution.
#[derive(Serialize, Deserialize)]
pub struct ImporterNimbyUpdate {
    pub error: Option<String>,
}

// --- Add Station ---

#[derive(Serialize, Deserialize)]
pub struct AddStationInit {
    pub graph: RailwayGraph,
    pub suggested_name: String,
    pub last_added_station: Option<usize>,
    pub track_handedness: TrackHandedness,
    pub default_node_distance_grid_squares: f64,
}

#[derive(Serialize, Deserialize)]
pub enum AddStationResult {
    Add {
        name: String,
        is_passing_loop: bool,
        connect_to: Option<usize>,
        platforms: Vec<Platform>,
    },
    AddBatch {
        entries: Vec<crate::components::add_station_quick::QuickEntryStation>,
        connect_to: Option<usize>,
        platforms: Vec<Platform>,
        tracks: Vec<Track>,
    },
}

// --- Add Station Update (from map click) ---

#[derive(Serialize, Deserialize)]
pub struct AddStationUpdate {
    pub clicked_edge_idx: Option<usize>,
    pub from_station_name: Option<String>,
    pub to_station_name: Option<String>,
}

// --- Create View ---

#[derive(Serialize, Deserialize)]
pub struct CreateViewInit {
    pub graph: RailwayGraph,
}

#[derive(Serialize, Deserialize)]
pub enum CreateViewResult {
    Create { name: String, waypoints: Vec<usize> },
}

// --- Create View Update (from map click) ---

#[derive(Serialize, Deserialize)]
pub struct CreateViewUpdate {
    pub waypoints: Vec<usize>,
}

// --- Project Manager ---

#[derive(Serialize, Deserialize)]
pub struct ProjectManagerInit {
    pub current_project_id: String,
    pub current_project_bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum ProjectManagerResult {
    LoadProject(Vec<u8>),
    CreateProject(Vec<u8>),
}
