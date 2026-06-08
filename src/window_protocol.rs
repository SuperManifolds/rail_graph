//! Typed init/result structs for native window communication.
//! Each window type has a pair of structs: one for initialization data
//! sent from the main window, and one for results sent back.

use crate::models::{Track, TrackHandedness};
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
