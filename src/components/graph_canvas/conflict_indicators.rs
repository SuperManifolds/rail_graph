use super::types::GraphDimensions;
use crate::models::Node;
use crate::conflict::Conflict;
use crate::constants::BASE_MIDNIGHT;
use crate::theme::Theme;
use web_sys::{CanvasRenderingContext2d, Path2d};

const CONFLICT_TRIANGLE_SIZE: f64 = 15.0;
const CONFLICT_TRIANGLE_FILL: &str = "rgba(255, 200, 0, 0.9)";
const CONFLICT_TRIANGLE_STROKE: &str = "rgba(0, 0, 0, 0.8)";
const CONFLICT_LABEL_FONT_SIZE: f64 = 9.0;
const CONFLICT_LABEL_OFFSET: f64 = 5.0;
const CONFLICT_WARNING_FONT_SIZE: f64 = 14.0;
const MAX_CONFLICTS_DISPLAYED: usize = 9999;

const BLOCK_FILL_OPACITY: &str = "33";
const BLOCK_STROKE_OPACITY: &str = "99";
const BLOCK_BORDER_WIDTH: f64 = 1.0;

struct Palette {
    exclamation: &'static str,
    label: &'static str,
    warning: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    exclamation: "#000",
    label: "rgba(255, 255, 255, 0.9)",
    warning: "rgba(255, 0, 0, 0.8)",
};

const LIGHT_PALETTE: Palette = Palette {
    exclamation: "#000",
    label: "rgba(0, 0, 0, 0.9)",
    warning: "rgba(200, 0, 0, 0.8)",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::too_many_arguments)]
pub fn draw_conflict_highlights(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflicts: &[&Conflict],
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    station_idx_map: &std::collections::HashMap<usize, usize>,
    theme: Theme,
) {
    let palette = get_palette(theme);
    let size = CONFLICT_TRIANGLE_SIZE / zoom_level;
    let bar_width = 1.5 / zoom_level;
    let bar_height = 6.0 / zoom_level;
    let dot_radius = 1.0 / zoom_level;

    // Create reusable Path2D objects for the triangle shape centered at origin
    let triangle_path = Path2d::new().ok();
    if let Some(ref path) = triangle_path {
        path.move_to(0.0, -size);
        path.line_to(-size * 0.866, size * 0.5);
        path.line_to(size * 0.866, size * 0.5);
        path.close_path();
    }

    // Create reusable Path2D for the exclamation dot
    let dot_path = Path2d::new().ok();
    if let Some(ref path) = dot_path {
        let _ = path.arc(0.0, 4.0 / zoom_level, dot_radius, 0.0, std::f64::consts::PI * 2.0);
    }

    // Build edge index -> view position map once (O(m) where m = edges in view)
    let edge_to_pos: std::collections::HashMap<usize, usize> = view_edge_path.iter()
        .enumerate()
        .map(|(pos, &edge)| (edge, pos))
        .collect();

    // Set styles once
    ctx.set_line_width(1.5 / zoom_level);

    // Draw each marker by translating and stamping the paths
    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        // Use edge-based matching for track conflicts (O(1) lookup), HashMap fallback for platform conflicts
        let (display_idx1, display_idx2) = if let Some(edge_idx) = conflict.edge_index {
            // O(1) lookup in pre-built HashMap
            if let Some(&pos) = edge_to_pos.get(&edge_idx) {
                (pos, pos + 1)
            } else {
                continue; // Edge not in current view
            }
        } else {
            // Fallback to station HashMap for platform conflicts
            let Some(&idx1) = station_idx_map.get(&conflict.station1_idx) else {
                continue;
            };
            let Some(&idx2) = station_idx_map.get(&conflict.station2_idx) else {
                continue;
            };
            (idx1, idx2)
        };

        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        // Interpolate Y position between the two stations based on conflict.position
        // Note: station_y_positions include the original TOP_MARGIN, but we're in a transformed
        // coordinate system where the origin has been moved by TOP_MARGIN, so we need to subtract it
        let y1 = station_y_positions[display_idx1] - super::canvas::TOP_MARGIN;
        let y2 = station_y_positions[display_idx2] - super::canvas::TOP_MARGIN;
        let y = y1 + (conflict.position * (y2 - y1));

        ctx.save();
        ctx.translate(x, y).ok();

        // Draw triangle using Path2D
        if let Some(ref path) = triangle_path {
            ctx.set_fill_style_str(CONFLICT_TRIANGLE_FILL);
            ctx.set_stroke_style_str(CONFLICT_TRIANGLE_STROKE);
            ctx.fill_with_path_2d(path);
            ctx.stroke_with_path(path);
        }

        // Draw exclamation bar
        ctx.set_fill_style_str(palette.exclamation);
        ctx.fill_rect(-bar_width / 2.0, -bar_height / 2.0 - 1.0 / zoom_level, bar_width, bar_height);

        // Draw exclamation dot using Path2D
        if let Some(ref path) = dot_path {
            ctx.fill_with_path_2d(path);
        }

        ctx.restore();
    }

    // Only draw labels when zoomed in enough (zoom level > 2.0)
    if zoom_level > 2.0 {
        // Set font and color for labels once
        ctx.set_fill_style_str(palette.label);
        ctx.set_font(&format!(
            "{}px monospace",
            CONFLICT_LABEL_FONT_SIZE / zoom_level
        ));

        // Draw all labels
        for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
            // Use edge-based matching for track conflicts (O(1) lookup), HashMap fallback for platform conflicts
            let (display_idx1, display_idx2) = if let Some(edge_idx) = conflict.edge_index {
                // O(1) lookup in pre-built HashMap
                if let Some(&pos) = edge_to_pos.get(&edge_idx) {
                    (pos, pos + 1)
                } else {
                    continue; // Edge not in current view
                }
            } else {
                // Fallback to station HashMap for platform conflicts
                let Some(&idx1) = station_idx_map.get(&conflict.station1_idx) else {
                    continue;
                };
                let Some(&idx2) = station_idx_map.get(&conflict.station2_idx) else {
                    continue;
                };
                (idx1, idx2)
            };

            let time_fraction = time_to_fraction(conflict.time);
            let x = dims.left_margin + (time_fraction * dims.hour_width);

            // Interpolate Y position between the two stations based on conflict.position
            // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
            let y1 = station_y_positions[display_idx1] - super::canvas::TOP_MARGIN;
            let y2 = station_y_positions[display_idx2] - super::canvas::TOP_MARGIN;
            let y = y1 + (conflict.position * (y2 - y1));

            let label = format!("{} × {}", conflict.journey1_id, conflict.journey2_id);
            let _ = ctx.fill_text(&label, x + size + CONFLICT_LABEL_OFFSET / zoom_level, y);
        }
    }

    // If there are more conflicts than displayed, show a count
    if conflicts.len() > MAX_CONFLICTS_DISPLAYED {
        ctx.set_fill_style_str(palette.warning);
        ctx.set_font(&format!(
            "bold {}px monospace",
            CONFLICT_WARNING_FONT_SIZE / zoom_level
        ));
        let warning_text = format!(
            "⚠ {} more conflicts not shown",
            conflicts.len() - MAX_CONFLICTS_DISPLAYED
        );
        let _ = ctx.fill_text(&warning_text, 10.0, dims.top_margin - 10.0);
    }
}

#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
#[must_use]
pub fn check_conflict_hover(
    mouse_x: f64,
    mouse_y: f64,
    conflicts: &[Conflict],
    _stations: &[(petgraph::stable_graph::NodeIndex, Node)],
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    dims: &super::types::GraphDimensions,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    pan_offset_y: f64,
    station_idx_map: &std::collections::HashMap<usize, usize>,
) -> Option<(Conflict, f64, f64)> {
    use crate::time::time_to_fraction;

    // Check if mouse is within the graph area first
    if mouse_x < dims.left_margin
        || mouse_x > dims.left_margin + dims.graph_width
        || mouse_y < dims.top_margin
        || mouse_y > dims.top_margin + dims.graph_height
    {
        return None;
    }

    // Build edge index -> view position map once (O(m) where m = edges in view)
    let edge_to_pos: std::collections::HashMap<usize, usize> = view_edge_path.iter()
        .enumerate()
        .map(|(pos, &edge)| (edge, pos))
        .collect();

    for conflict in conflicts {
        // Use edge-based matching for track conflicts (O(1) lookup), HashMap fallback for platform conflicts
        let (display_idx1, display_idx2) = if let Some(edge_idx) = conflict.edge_index {
            // O(1) lookup in pre-built HashMap
            if let Some(&pos) = edge_to_pos.get(&edge_idx) {
                (pos, pos + 1)
            } else {
                continue; // Edge not in current view
            }
        } else {
            // Fallback to station HashMap for platform conflicts
            let Some(&idx1) = station_idx_map.get(&conflict.station1_idx) else {
                continue;
            };
            let Some(&idx2) = station_idx_map.get(&conflict.station2_idx) else {
                continue;
            };
            (idx1, idx2)
        };

        // Calculate conflict position in screen coordinates
        // The canvas uses: translate(left_margin, top_margin) + translate(pan) + scale(zoom)
        let time_fraction = time_to_fraction(conflict.time);
        let total_hours = 48.0;
        let hour_width = dims.graph_width / total_hours;

        // Position in zoomed coordinate system (before translation)
        let x_in_zoomed = time_fraction * hour_width;

        // Interpolate Y position between the two stations
        let y1 = station_y_positions[display_idx1] - dims.top_margin;
        let y2 = station_y_positions[display_idx2] - dims.top_margin;
        let y_in_zoomed = y1 + (conflict.position * (y2 - y1));

        // Transform to screen coordinates
        let screen_x = dims.left_margin + (x_in_zoomed * zoom_level * zoom_level_x) + pan_offset_x;
        let screen_y = dims.top_margin + (y_in_zoomed * zoom_level) + pan_offset_y;

        // Check if mouse is within conflict marker bounds
        let size = CONFLICT_TRIANGLE_SIZE;
        if mouse_x >= screen_x - size
            && mouse_x <= screen_x + size
            && mouse_y >= screen_y - size
            && mouse_y <= screen_y + size
        {
            // Return the conflict marker's screen position, not the mouse position
            return Some((conflict.clone(), screen_x, screen_y));
        }
    }

    None
}

#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn draw_block_violation_visualization(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflict: &Conflict,
    train_journeys: &[&crate::train_journey::TrainJourney],
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    station_idx_map: &std::collections::HashMap<usize, usize>,
) {
    // Use the segment times stored in the conflict
    if let (Some((s1_start, s1_end)), Some((s2_start, s2_end))) =
        (conflict.segment1_times, conflict.segment2_times) {

        // For conflicts with edge_index (block/track conflicts), use edge-based matching
        let (display_idx1, display_idx2) = if let Some(edge_idx) = conflict.edge_index {
            // Find this edge in the view edge path
            if let Some(pos) = view_edge_path.iter().position(|&e| e == edge_idx) {
                (pos, pos + 1)
            } else {
                return; // Edge not in current view
            }
        } else {
            // Fallback to HashMap for platform conflicts (should not have visualization blocks)
            let Some(&display_idx1) = station_idx_map.get(&conflict.station1_idx) else {
                return; // Station not in current view
            };
            let Some(&display_idx2) = station_idx_map.get(&conflict.station2_idx) else {
                return; // Station not in current view
            };
            (display_idx1, display_idx2)
        };

        // Find the journeys to get their colors
        let journey1 = train_journeys.iter().find(|j| j.train_number == conflict.journey1_id);
        let journey2 = train_journeys.iter().find(|j| j.train_number == conflict.journey2_id);

        // Get colors from journeys (hex format like #FF0000)
        let color1 = journey1.map_or("#FF0000", |j| j.color.as_str());
        let color2 = journey2.map_or("#0000FF", |j| j.color.as_str());

        // Convert hex to rgba with transparency
        let fill1 = format!("{color1}{BLOCK_FILL_OPACITY}");
        let fill2 = format!("{color2}{BLOCK_FILL_OPACITY}");

        // Draw first journey's block
        draw_block_rectangle(
            ctx,
            dims,
            (s1_start, s1_end),
            (display_idx1, display_idx2),
            station_y_positions,
            zoom_level,
            time_to_fraction,
            (&fill1, color1),
        );

        // Draw second journey's block
        draw_block_rectangle(
            ctx,
            dims,
            (s2_start, s2_end),
            (display_idx1, display_idx2),
            station_y_positions,
            zoom_level,
            time_to_fraction,
            (&fill2, color2),
        );
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_journey_blocks(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    journey: &crate::train_journey::TrainJourney,
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    view_nodes: &[(petgraph::stable_graph::NodeIndex, crate::models::Node)],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    // Get journey color
    let color = journey.color.as_str();
    let fill_color = format!("{color}{BLOCK_FILL_OPACITY}");
    let stroke_color = format!("{color}{BLOCK_STROKE_OPACITY}");

    // Use the same edge-based matching as train journeys (handles bidirectional traversal)
    let station_positions = super::train_journeys::match_journey_stations_to_view_by_edges(
        &journey.segments,
        &journey.station_times,
        view_edge_path,
        view_nodes,
    );

    // Draw blocks for each segment
    for i in 0..journey.segments.len() {
        // Bounds check: ensure we have both start and end stations for this segment
        if i + 1 >= journey.station_times.len() {
            break;
        }

        let (_node_from, _arrival_from, departure_from) = journey.station_times[i];
        let (_node_to, arrival_to, _departure_to) = journey.station_times[i + 1];

        // Skip blocks that end before the week start (day -1 Sunday)
        if arrival_to < BASE_MIDNIGHT {
            continue;
        }

        // Get the matched view positions for this segment
        if let (Some(start_idx), Some(end_idx)) = (
            station_positions.get(i).and_then(|&opt| opt),
            station_positions.get(i + 1).and_then(|&opt| opt),
        ) {
            draw_block_rectangle(
                ctx,
                dims,
                (departure_from, arrival_to),
                (start_idx, end_idx),
                station_y_positions,
                zoom_level,
                time_to_fraction,
                (&fill_color, &stroke_color),
            );
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_block_rectangle(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    times: (chrono::NaiveDateTime, chrono::NaiveDateTime),
    stations: (usize, usize),
    station_y_positions: &[f64],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    colors: (&str, &str),
) {
    let (start_time, end_time) = times;
    let (start_station_idx, end_station_idx) = stations;
    let (fill_color, stroke_color) = colors;

    let x1 = dims.left_margin + (time_to_fraction(start_time) * dims.hour_width);
    let x2 = dims.left_margin + (time_to_fraction(end_time) * dims.hour_width);
    // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
    let y1 = station_y_positions[start_station_idx] - super::canvas::TOP_MARGIN;
    let y2 = station_y_positions[end_station_idx] - super::canvas::TOP_MARGIN;

    let width = x2 - x1;
    let height = y2 - y1;

    // Draw rectangle
    ctx.set_fill_style_str(fill_color);
    ctx.fill_rect(x1, y1, width, height);

    // Draw border with zoom-adjusted line width
    ctx.set_stroke_style_str(stroke_color);
    ctx.set_line_width(BLOCK_BORDER_WIDTH / zoom_level);
    ctx.stroke_rect(x1, y1, width, height);
}

