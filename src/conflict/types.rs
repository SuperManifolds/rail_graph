//! Core types for conflict detection.

use crate::models::{Junctions, RailwayGraph, TrackDirection};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    HeadOn,            // Trains meeting on same track, opposite directions
    Overtaking,        // Train catching up on same track, same direction
    BlockViolation,    // Two trains in same single-track block simultaneously
    PlatformViolation, // Two trains using same platform at same time
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    pub time: NaiveDateTime,
    pub position: f64, // Position between stations (0.0 to 1.0)
    pub station1_idx: usize,
    pub station2_idx: usize,
    pub journey1_id: String,
    pub journey2_id: String,
    pub conflict_type: ConflictType,
    // For block violations: store the time ranges of the two segments
    pub segment1_times: Option<(NaiveDateTime, NaiveDateTime)>,
    pub segment2_times: Option<(NaiveDateTime, NaiveDateTime)>,
    // For platform violations: store the platform index
    pub platform_idx: Option<usize>,
    // Edge index for block/track conflicts (None for platform conflicts)
    pub edge_index: Option<usize>,
    // Whether at least one train has inherited timing (uncertain exact time)
    pub timing_uncertain: bool,
    // For platform violations: actual arrival/departure times without buffer (for visualization)
    pub actual1_times: Option<(NaiveDateTime, NaiveDateTime)>,
    pub actual2_times: Option<(NaiveDateTime, NaiveDateTime)>,
}

impl Conflict {
    /// Format a human-readable message describing the conflict (without timestamp)
    /// For `PlatformViolation` conflicts, caller should use `format_platform_message` instead for better performance
    #[must_use]
    pub fn format_message(&self, station1_name: &str, station2_name: &str) -> String {
        let base_message = match self.conflict_type {
            ConflictType::PlatformViolation => {
                format!(
                    "{} conflicts with {} at {} Platform ?",
                    self.journey1_id, self.journey2_id, station1_name
                )
            }
            ConflictType::HeadOn => {
                format!(
                    "{} conflicts with {} between {} and {}",
                    self.journey1_id, self.journey2_id, station1_name, station2_name
                )
            }
            ConflictType::Overtaking => {
                format!(
                    "{} overtakes {} between {} and {}",
                    self.journey2_id, self.journey1_id, station1_name, station2_name
                )
            }
            ConflictType::BlockViolation => {
                format!(
                    "{} block violation with {} between {} and {}",
                    self.journey1_id, self.journey2_id, station1_name, station2_name
                )
            }
        };

        if self.timing_uncertain {
            format!("⚠️ {base_message} (timing uncertain - at least one train has no explicit time, but conflict must be assumed)")
        } else {
            base_message
        }
    }

    /// Format platform violation message with platform name provided (avoids graph lookup)
    #[must_use]
    pub fn format_platform_message(&self, station1_name: &str, platform_name: &str) -> String {
        let base_message = format!(
            "{} conflicts with {} at {} Platform {}",
            self.journey1_id, self.journey2_id, station1_name, platform_name
        );

        if self.timing_uncertain {
            format!("⚠️ {base_message} (timing uncertain - at least one train has no explicit time, but conflict must be assumed)")
        } else {
            base_message
        }
    }

    /// Get a short name for the conflict type
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self.conflict_type {
            ConflictType::HeadOn => "Head-on Conflict",
            ConflictType::Overtaking => "Overtaking",
            ConflictType::BlockViolation => "Block Violation",
            ConflictType::PlatformViolation => "Platform Violation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StationCrossing {
    pub time: NaiveDateTime,
    pub station_idx: usize,
    pub journey1_id: String,
    pub journey2_id: String,
}

/// Serializable context for conflict detection (no references, no complex graph types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableConflictContext {
    /// Maps station `NodeIndex` (as usize) to display index
    pub station_indices: HashMap<usize, usize>,
    /// Maps edge index -> (`is_single_track_bidirectional`, `track_count`)
    pub edge_info: HashMap<usize, (bool, usize)>,
    /// Maps (`edge_index`, `track_index`) -> `is_bidirectional`
    pub track_directions: HashMap<(usize, usize), bool>,
    /// Set of junction node indices (as usize)
    pub junctions: std::collections::HashSet<usize>,
    pub station_margin_secs: i64,
    pub minimum_separation_secs: i64,
    pub ignore_same_direction_platform_conflicts: bool,
}

impl SerializableConflictContext {
    /// Build serializable context from a `RailwayGraph`
    #[must_use]
    pub fn from_graph(
        graph: &RailwayGraph,
        station_indices: HashMap<petgraph::stable_graph::NodeIndex, usize>,
        station_margin: chrono::Duration,
        minimum_separation: chrono::Duration,
        ignore_same_direction_platform_conflicts: bool,
    ) -> Self {
        use petgraph::visit::{EdgeRef, IntoEdgeReferences};

        // Extract edge information and track directions
        let mut edge_info = HashMap::new();
        let mut track_directions = HashMap::new();
        for edge in graph.graph.edge_references() {
            let edge_idx = edge.id().index();
            let track_segment = edge.weight();
            let is_single_bidirectional = track_segment.tracks.len() == 1
                && matches!(track_segment.tracks[0].direction, TrackDirection::Bidirectional);
            edge_info.insert(edge_idx, (is_single_bidirectional, track_segment.tracks.len()));

            // Store direction for each track
            for (track_idx, track) in track_segment.tracks.iter().enumerate() {
                let is_bidirectional = matches!(track.direction, TrackDirection::Bidirectional);
                track_directions.insert((edge_idx, track_idx), is_bidirectional);
            }
        }

        // Extract junction information
        let junctions = graph.graph.node_indices()
            .filter(|&idx| graph.is_junction(idx))
            .map(petgraph::prelude::NodeIndex::index)
            .collect();

        // Convert station_indices to use usize keys
        let station_indices = station_indices.into_iter()
            .map(|(k, v)| (k.index(), v))
            .collect();

        Self {
            station_indices,
            edge_info,
            track_directions,
            junctions,
            station_margin_secs: station_margin.num_seconds(),
            minimum_separation_secs: minimum_separation.num_seconds(),
            ignore_same_direction_platform_conflicts,
        }
    }
}
