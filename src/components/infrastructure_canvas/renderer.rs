use crate::models::RailwayGraph;
use super::{track_renderer, station_renderer};
use web_sys::CanvasRenderingContext2d;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{HashSet, HashMap};

type EdgeSegments = Vec<((f64, f64), (f64, f64))>;
type LabelPositionCache = HashMap<NodeIndex, station_renderer::CachedLabelPosition>;

/// Topology-dependent cached data (exported for use by `infrastructure_view`)
#[derive(Clone, Default)]
pub struct TopologyCache {
    pub topology: (usize, usize),
    pub avoidance_offsets: HashMap<EdgeIndex, (f64, f64)>,
    pub edge_segments: HashMap<EdgeIndex, EdgeSegments>,
    /// Cached label positions (zoom level, positions)
    pub label_cache: Option<(f64, LabelPositionCache)>,
}

const CANVAS_BACKGROUND_COLOR: &str = "#0a0a0a";
const EMPTY_MESSAGE_COLOR: &str = "#666";
const EMPTY_MESSAGE_FONT: &str = "16px sans-serif";
const EMPTY_MESSAGE_TEXT: &str = "No stations in network";
const EMPTY_MESSAGE_OFFSET_X: f64 = 80.0;

const GRID_SIZE: f64 = 30.0; // Must match auto_layout.rs GRID_SIZE
const GRID_COLOR: &str = "#141414";
const GRID_LINE_WIDTH: f64 = 0.5;

/// Build topology cache with avoidance offsets and edge segments
#[must_use]
pub fn build_topology_cache(graph: &RailwayGraph) -> TopologyCache {
    use crate::models::Stations;

    let topology = (graph.graph.node_count(), graph.graph.edge_count());
    let mut avoidance_offsets = HashMap::new();
    let mut edge_segments = HashMap::new();

    // Precompute avoidance offsets and segments for all edges
    for edge in graph.graph.edge_references() {
        let edge_id = edge.id();
        let source = edge.source();
        let target = edge.target();

        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Calculate avoidance offset
        let offset = track_renderer::calculate_avoidance_offset(graph, pos1, pos2, source, target);
        avoidance_offsets.insert(edge_id, offset);

        // Calculate segments
        let segments = track_renderer::get_segments_for_edge(graph, source, target, pos1, pos2);
        edge_segments.insert(edge_id, segments);
    }

    TopologyCache {
        topology,
        avoidance_offsets,
        edge_segments,
        label_cache: None,
    }
}

/// Draw a subtle grid pattern to show snap points
fn draw_grid(
    ctx: &CanvasRenderingContext2d,
    width: f64,
    height: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
) {
    ctx.save();

    ctx.set_stroke_style_str(GRID_COLOR);
    ctx.set_line_width(GRID_LINE_WIDTH);

    // Calculate visible world bounds
    let left = -pan_x / zoom;
    let top = -pan_y / zoom;
    let right = (width - pan_x) / zoom;
    let bottom = (height - pan_y) / zoom;

    // Round to nearest grid line
    let start_x = (left / GRID_SIZE).floor() * GRID_SIZE;
    let start_y = (top / GRID_SIZE).floor() * GRID_SIZE;

    // Apply transformations
    let _ = ctx.translate(pan_x, pan_y);
    let _ = ctx.scale(zoom, zoom);

    ctx.begin_path();

    // Draw vertical lines
    let mut x = start_x;
    while x <= right {
        ctx.move_to(x, top);
        ctx.line_to(x, bottom);
        x += GRID_SIZE;
    }

    // Draw horizontal lines
    let mut y = start_y;
    while y <= bottom {
        ctx.move_to(left, y);
        ctx.line_to(right, y);
        y += GRID_SIZE;
    }

    ctx.stroke();
    ctx.restore();
}

#[allow(clippy::too_many_arguments)]
pub fn draw_infrastructure(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    (width, height): (f64, f64),
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &HashSet<EdgeIndex>,
    cache: &mut TopologyCache,
    is_zooming: bool,
    preview_station_position: Option<(f64, f64)>,
) {
    // Clear canvas
    ctx.set_fill_style_str(CANVAS_BACKGROUND_COLOR);
    ctx.fill_rect(0.0, 0.0, width, height);

    // Draw grid
    draw_grid(ctx, width, height, zoom, pan_x, pan_y);

    if graph.graph.node_count() == 0 {
        // Show message if no stations
        ctx.set_fill_style_str(EMPTY_MESSAGE_COLOR);
        ctx.set_font(EMPTY_MESSAGE_FONT);
        let _ = ctx.fill_text(EMPTY_MESSAGE_TEXT, width / 2.0 - EMPTY_MESSAGE_OFFSET_X, height / 2.0);
        return;
    }

    // Save context and apply transformations
    ctx.save();
    let _ = ctx.translate(pan_x, pan_y);
    let _ = ctx.scale(zoom, zoom);

    // Draw tracks first so they're behind nodes (using cached avoidance offsets)
    track_renderer::draw_tracks(ctx, graph, zoom, highlighted_edges, &cache.avoidance_offsets);

    // Draw stations and junctions on top (with label cache)
    station_renderer::draw_stations_with_cache(ctx, graph, zoom, selected_stations, highlighted_edges, cache, is_zooming);

    // Draw preview station if position is set
    if let Some((x, y)) = preview_station_position {
        const PREVIEW_NODE_RADIUS: f64 = 8.0;
        const PREVIEW_STROKE_COLOR: &str = "#4a9eff"; // Same blue as stations
        const PREVIEW_FILL_COLOR: &str = "#2a2a2a"; // Same as NODE_FILL_COLOR
        const PREVIEW_ALPHA: f64 = 0.5;

        ctx.save();
        ctx.set_global_alpha(PREVIEW_ALPHA);
        ctx.set_fill_style_str(PREVIEW_FILL_COLOR);
        ctx.set_stroke_style_str(PREVIEW_STROKE_COLOR);
        ctx.set_line_width(2.0 / zoom);
        ctx.begin_path();
        let _ = ctx.arc(x, y, PREVIEW_NODE_RADIUS, 0.0, 2.0 * std::f64::consts::PI);
        ctx.fill();
        ctx.stroke();
        ctx.set_global_alpha(1.0);
        ctx.restore();
    }

    // Restore context
    ctx.restore();
}
