use crate::models::RailwayGraph;
use web_sys::CanvasRenderingContext2d;

const NODE_RADIUS: f64 = 8.0;

pub fn draw_stations(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
) {
    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(station) = graph.graph.node_weight(idx) else { continue };

        // Draw node circle with size and color based on passing loop status
        ctx.set_fill_style_str("#2a2a2a");
        let (border_color, radius) = if station.passing_loop {
            ("#888", NODE_RADIUS * 0.6)
        } else {
            ("#4a9eff", NODE_RADIUS)
        };
        ctx.set_stroke_style_str(border_color);
        ctx.set_line_width(2.0 / zoom);
        ctx.begin_path();
        let _ = ctx.arc(pos.0, pos.1, radius, 0.0, std::f64::consts::PI * 2.0);
        ctx.fill();
        ctx.stroke();

        // Draw station name (scale font size inversely with zoom)
        ctx.set_fill_style_str("#fff");
        let font_size = 14.0 / zoom;
        ctx.set_font(&format!("{}px sans-serif", font_size));
        let _ = ctx.fill_text(&station.name, pos.0 + NODE_RADIUS + 5.0, pos.1 + 5.0);
    }
}
