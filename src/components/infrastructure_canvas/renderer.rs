use crate::models::{Line, RailwayGraph, Junctions};
use crate::theme::Theme;
use super::{track_renderer, station_renderer, line_renderer, line_station_renderer, junction_renderer};
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
    /// Junction node indices for O(1) lookup
    pub junctions: HashSet<NodeIndex>,
    /// Station node indices for O(1) lookup
    pub stations: HashSet<NodeIndex>,
    /// Adjacency map: node -> (neighbor, `edge_index`) for branch identification
    pub adjacency: HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>>,
    /// Orphaned tracks: (edge, junction) -> set of track indices without connections
    pub orphaned_tracks: HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    /// Crossover intersection points: (edge, junction, `track_idx`) -> intersection point
    pub crossover_intersections: HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)>,
}

const EMPTY_MESSAGE_FONT: &str = "16px sans-serif";
const EMPTY_MESSAGE_TEXT: &str = "No stations in network";
const EMPTY_MESSAGE_OFFSET_X: f64 = 80.0;

const GRID_SIZE: f64 = 30.0;
const GRID_LINE_WIDTH: f64 = 0.5;

const SELECTION_BOX_LINE_WIDTH: f64 = 1.5;
const SELECTION_BOX_DASH_LENGTH: f64 = 5.0;

struct Palette {
    background: &'static str,
    empty_message: &'static str,
    grid: &'static str,
    selection_box_stroke: &'static str,
    selection_box_fill: &'static str,
    preview_stroke: &'static str,
    preview_fill: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    background: "#0a0a0a",
    empty_message: "#666",
    grid: "#141414",
    selection_box_stroke: "#4a9eff",
    selection_box_fill: "rgba(74, 158, 255, 0.1)",
    preview_stroke: "#4a9eff",
    preview_fill: "#2a2a2a",
};

const LIGHT_PALETTE: Palette = Palette {
    background: "#fafafa",
    empty_message: "#999",
    grid: "#ebebeb",
    selection_box_stroke: "#1976d2",
    selection_box_fill: "rgba(25, 118, 210, 0.08)",
    preview_stroke: "#1976d2",
    preview_fill: "#f0f0f0",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

/// Build topology cache with avoidance offsets and edge segments
#[must_use]
pub fn build_topology_cache(graph: &RailwayGraph) -> TopologyCache {
    use crate::models::Stations;

    let topology = (graph.graph.node_count(), graph.graph.edge_count());
    let mut avoidance_offsets = HashMap::new();
    let mut edge_segments = HashMap::new();
    let mut junctions = HashSet::new();
    let mut stations = HashSet::new();
    let mut adjacency: HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>> = HashMap::new();

    // Categorize nodes
    for idx in graph.graph.node_indices() {
        if graph.is_junction(idx) {
            junctions.insert(idx);
        } else {
            stations.insert(idx);
        }
    }

    // Build adjacency map and precompute edge data
    for edge in graph.graph.edge_references() {
        let edge_id = edge.id();
        let source = edge.source();
        let target = edge.target();

        // Add to adjacency map (bidirectional)
        adjacency.entry(source).or_default().push((target, edge_id));
        adjacency.entry(target).or_default().push((source, edge_id));

        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Calculate avoidance offset
        let offset = track_renderer::calculate_avoidance_offset(graph, pos1, pos2, source, target);
        avoidance_offsets.insert(edge_id, offset);

        // Calculate segments
        let segments = track_renderer::get_segments_for_edge(graph, source, target, pos1, pos2);
        edge_segments.insert(edge_id, segments);
    }

    // Calculate orphaned tracks (tracks without junction connections)
    let orphaned_tracks = junction_renderer::get_orphaned_tracks_map(graph);

    // Calculate crossover intersection points for orphaned tracks
    let crossover_intersections = junction_renderer::get_crossover_intersection_points(
        graph,
        &orphaned_tracks,
        &avoidance_offsets,
    );

    TopologyCache {
        topology,
        avoidance_offsets,
        edge_segments,
        label_cache: None,
        junctions,
        stations,
        adjacency,
        orphaned_tracks,
        crossover_intersections,
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
    palette: &Palette,
) {
    ctx.save();

    ctx.set_stroke_style_str(palette.grid);
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
    lines: &[Line],
    show_lines: bool,
    (width, height): (f64, f64),
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &HashSet<EdgeIndex>,
    cache: &mut TopologyCache,
    is_zooming: bool,
    preview_station_position: Option<(f64, f64)>,
    selection_box: Option<((f64, f64), (f64, f64))>,
    theme: Theme,
) {
    let palette = get_palette(theme);

    // Clear canvas
    ctx.set_fill_style_str(palette.background);
    ctx.fill_rect(0.0, 0.0, width, height);

    // Draw grid
    draw_grid(ctx, width, height, zoom, pan_x, pan_y, palette);

    if graph.graph.node_count() == 0 {
        // Show message if no stations
        ctx.set_fill_style_str(palette.empty_message);
        ctx.set_font(EMPTY_MESSAGE_FONT);
        let _ = ctx.fill_text(EMPTY_MESSAGE_TEXT, width / 2.0 - EMPTY_MESSAGE_OFFSET_X, height / 2.0);
        return;
    }

    // Calculate visible world bounds for viewport culling
    let viewport_left = -pan_x / zoom;
    let viewport_top = -pan_y / zoom;
    let viewport_right = (width - pan_x) / zoom;
    let viewport_bottom = (height - pan_y) / zoom;
    let viewport_bounds = (viewport_left, viewport_top, viewport_right, viewport_bottom);

    // Save context and apply transformations
    ctx.save();
    let _ = ctx.translate(pan_x, pan_y);
    let _ = ctx.scale(zoom, zoom);

    // Draw tracks or lines based on toggle (behind nodes)
    if show_lines {
        // Draw lines instead of tracks (use zoom=1.0 for constant size scaling)
        line_renderer::draw_lines(ctx, graph, lines, 1.0, &cache.avoidance_offsets, viewport_bounds, &cache.junctions, theme);
        // Draw custom station markers for line mode (use zoom=1.0 for constant size scaling)
        line_station_renderer::draw_line_stations(ctx, graph, lines, 1.0, viewport_bounds, &cache.label_cache, selected_stations, theme);
    } else {
        // Draw tracks (using cached avoidance offsets)
        track_renderer::draw_tracks(ctx, graph, zoom, highlighted_edges, &cache.avoidance_offsets, viewport_bounds, &cache.junctions, theme, &cache.orphaned_tracks, &cache.crossover_intersections);
    }

    // Draw stations and junctions on top (with label cache)
    // Use zoom=1.0 in line mode for constant size labels
    station_renderer::draw_stations_with_cache(ctx, graph, lines, if show_lines { 1.0 } else { zoom }, selected_stations, highlighted_edges, cache, is_zooming, viewport_bounds, show_lines, theme);

    // Draw preview station if position is set
    if let Some((x, y)) = preview_station_position {
        const PREVIEW_NODE_RADIUS: f64 = 8.0;
        const PREVIEW_ALPHA: f64 = 0.5;

        ctx.save();
        ctx.set_global_alpha(PREVIEW_ALPHA);
        ctx.set_fill_style_str(palette.preview_fill);
        ctx.set_stroke_style_str(palette.preview_stroke);
        ctx.set_line_width(2.0 / zoom);
        ctx.begin_path();
        let _ = ctx.arc(x, y, PREVIEW_NODE_RADIUS, 0.0, 2.0 * std::f64::consts::PI);
        ctx.fill();
        ctx.stroke();
        ctx.set_global_alpha(1.0);
        ctx.restore();
    }

    // Draw selection box if dragging
    if let Some((start, end)) = selection_box {
        let min_x = start.0.min(end.0);
        let max_x = start.0.max(end.0);
        let min_y = start.1.min(end.1);
        let max_y = start.1.max(end.1);
        let width_box = max_x - min_x;
        let height_box = max_y - min_y;

        ctx.set_stroke_style_str(palette.selection_box_stroke);
        ctx.set_fill_style_str(palette.selection_box_fill);
        ctx.set_line_width(SELECTION_BOX_LINE_WIDTH / zoom);
        let dash_array = js_sys::Array::of2(
            &wasm_bindgen::JsValue::from(SELECTION_BOX_DASH_LENGTH / zoom),
            &wasm_bindgen::JsValue::from(SELECTION_BOX_DASH_LENGTH / zoom)
        );
        let _ = ctx.set_line_dash(&dash_array);
        ctx.stroke_rect(min_x, min_y, width_box, height_box);
        ctx.fill_rect(min_x, min_y, width_box, height_box);
        // Reset line dash
        let _ = ctx.set_line_dash(&js_sys::Array::new());
    }

    // Restore context
    ctx.restore();
}
