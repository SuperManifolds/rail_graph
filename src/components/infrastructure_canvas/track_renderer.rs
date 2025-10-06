use crate::models::RailwayGraph;
use petgraph::visit::EdgeRef;
use web_sys::CanvasRenderingContext2d;

const TRACK_SPACING: f64 = 3.0;

pub fn draw_tracks(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
) {
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        let track_count = edge.weight().tracks.len();

        if track_count == 0 {
            continue;
        }

        // Calculate perpendicular offset for parallel tracks
        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();
        let nx = -dy / len;
        let ny = dx / len;

        ctx.set_line_width(2.0 / zoom);

        if track_count == 1 {
            // Single track - draw in center
            ctx.set_stroke_style_str("#444");
            ctx.begin_path();
            ctx.move_to(pos1.0, pos1.1);
            ctx.line_to(pos2.0, pos2.1);
            ctx.stroke();
        } else {
            // Multiple tracks - distribute evenly
            let total_width = (track_count - 1) as f64 * TRACK_SPACING;
            let start_offset = -total_width / 2.0;

            for (i, _track) in edge.weight().tracks.iter().enumerate() {
                let offset = start_offset + (i as f64 * TRACK_SPACING);
                let ox = nx * offset;
                let oy = ny * offset;

                ctx.set_stroke_style_str("#555");
                ctx.begin_path();
                ctx.move_to(pos1.0 + ox, pos1.1 + oy);
                ctx.line_to(pos2.0 + ox, pos2.1 + oy);
                ctx.stroke();
            }
        }
    }
}
