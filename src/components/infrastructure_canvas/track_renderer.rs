use crate::models::RailwayGraph;
use petgraph::visit::EdgeRef;
use web_sys::CanvasRenderingContext2d;

const TRACK_OFFSET: f64 = 3.0;

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

        let is_double = edge.weight().double_tracked;

        if is_double {
            // Draw two parallel lines for double track
            let dx = pos2.0 - pos1.0;
            let dy = pos2.1 - pos1.1;
            let len = (dx * dx + dy * dy).sqrt();
            let nx = -dy / len * TRACK_OFFSET;
            let ny = dx / len * TRACK_OFFSET;

            ctx.set_stroke_style_str("#555");
            ctx.set_line_width(2.0 / zoom);

            // First track
            ctx.begin_path();
            ctx.move_to(pos1.0 + nx, pos1.1 + ny);
            ctx.line_to(pos2.0 + nx, pos2.1 + ny);
            ctx.stroke();

            // Second track
            ctx.begin_path();
            ctx.move_to(pos1.0 - nx, pos1.1 - ny);
            ctx.line_to(pos2.0 - nx, pos2.1 - ny);
            ctx.stroke();
        } else {
            // Single track
            ctx.set_stroke_style_str("#444");
            ctx.set_line_width(2.0 / zoom);
            ctx.begin_path();
            ctx.move_to(pos1.0, pos1.1);
            ctx.line_to(pos2.0, pos2.1);
            ctx.stroke();
        }
    }
}
