use crate::models::{RailwayGraph, Stations};
use super::track_renderer;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::HashMap;

const STATION_CLICK_THRESHOLD: f64 = 15.0;
const TRACK_CLICK_THRESHOLD: f64 = 8.0;

type TrackSegments = Vec<((f64, f64), (f64, f64))>;

#[must_use]
pub fn find_station_at_position(graph: &RailwayGraph, x: f64, y: f64) -> Option<NodeIndex> {
    for idx in graph.graph.node_indices() {
        if let Some(pos) = graph.get_station_position(idx) {
            let dx = pos.0 - x;
            let dy = pos.1 - y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= STATION_CLICK_THRESHOLD {
                return Some(idx);
            }
        }
    }

    None
}

fn distance_to_segment(point: (f64, f64), seg_start: (f64, f64), seg_end: (f64, f64)) -> f64 {
    let dx = seg_end.0 - seg_start.0;
    let dy = seg_end.1 - seg_start.1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        // Degenerate segment
        let dx = point.0 - seg_start.0;
        let dy = point.1 - seg_start.1;
        return (dx * dx + dy * dy).sqrt();
    }

    // Calculate projection parameter t
    let t = ((point.0 - seg_start.0) * dx + (point.1 - seg_start.1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    // Find closest point on segment
    let closest_x = seg_start.0 + t * dx;
    let closest_y = seg_start.1 + t * dy;

    // Calculate distance
    let dist_x = point.0 - closest_x;
    let dist_y = point.1 - closest_y;
    (dist_x * dist_x + dist_y * dist_y).sqrt()
}

#[must_use]
pub fn find_track_at_position(graph: &RailwayGraph, x: f64, y: f64) -> Option<EdgeIndex> {
    // Build a mapping from segments to edge indices
    // For each edge, get its actual rendered segments (including avoidance paths)
    let mut edge_segments: HashMap<EdgeIndex, TrackSegments> = HashMap::new();

    // Use same logic as track renderer to get actual segments
    for edge in graph.graph.edge_references() {
        let edge_id = edge.id();
        let source = edge.source();
        let target = edge.target();

        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Check if we need avoidance (using same logic as track_renderer)
        let segments = track_renderer::get_segments_for_edge(graph, source, target, pos1, pos2);
        edge_segments.insert(edge_id, segments);
    }

    // Check each segment for each edge
    for (edge_id, segments) in edge_segments {
        for (seg_start, seg_end) in segments {
            let dist = distance_to_segment((x, y), seg_start, seg_end);
            if dist <= TRACK_CLICK_THRESHOLD {
                return Some(edge_id);
            }
        }
    }

    None
}
