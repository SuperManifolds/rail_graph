use nimby_graph::import::{import_nimby_lines, parse_nimby_json, NimbyImportConfig};
use nimby_graph::models::{RailwayGraph, Stations, TrackHandedness};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::fmt::Write as FmtWrite;
use std::fs;

fn generate_svg(graph: &RailwayGraph) -> String {
    // Collect positioned stations
    let stations: Vec<_> = graph
        .graph
        .node_indices()
        .filter_map(|idx| {
            let name = graph.graph[idx].display_name().clone();
            let pos = graph.get_station_position(idx)?;
            let is_passing_loop = graph.graph[idx]
                .as_station()
                .is_some_and(|s| s.passing_loop);
            Some((idx, name, pos, is_passing_loop))
        })
        .collect();

    if stations.is_empty() {
        return String::from("<svg></svg>");
    }

    // Find bounds
    let min_x = stations.iter().map(|(_, _, (x, _), _)| *x).fold(f64::MAX, f64::min);
    let max_x = stations.iter().map(|(_, _, (x, _), _)| *x).fold(f64::MIN, f64::max);
    let min_y = stations.iter().map(|(_, _, (_, y), _)| *y).fold(f64::MAX, f64::min);
    let max_y = stations.iter().map(|(_, _, (_, y), _)| *y).fold(f64::MIN, f64::max);

    // Fixed output size, scale content to fit
    let output_size = 3000.0;
    let padding = 100.0;
    let content_w = max_x - min_x;
    let content_h = max_y - min_y;
    let scale = (output_size - padding * 2.0) / content_w.max(content_h).max(1.0);
    let width = output_size;
    let height = output_size * (content_h / content_w.max(1.0)).max(0.5);

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}">"#
    );
    let _ = writeln!(
        svg,
        r##"<rect width="{width}" height="{height}" fill="#1a1a2e"/>"##
    );

    // Draw edges
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();

        let Some(src_pos) = graph.get_station_position(source) else {
            continue;
        };
        let Some(tgt_pos) = graph.get_station_position(target) else {
            continue;
        };

        let x1 = (src_pos.0 - min_x + padding) * scale;
        let y1 = (src_pos.1 - min_y + padding) * scale;
        let x2 = (tgt_pos.0 - min_x + padding) * scale;
        let y2 = (tgt_pos.1 - min_y + padding) * scale;

        let _ = writeln!(
            svg,
            r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#4a4a6a" stroke-width="3"/>"##
        );
    }

    // Determine which stations sit on horizontal edges (for label rotation)
    let mut on_horizontal: std::collections::HashSet<petgraph::stable_graph::NodeIndex> =
        std::collections::HashSet::new();
    for edge in graph.graph.edge_references() {
        let Some(sp) = graph.get_station_position(edge.source()) else { continue };
        let Some(tp) = graph.get_station_position(edge.target()) else { continue };
        if (sp.1 - tp.1).abs() < 1.0 && (sp.0 - tp.0).abs() > 1.0 {
            on_horizontal.insert(edge.source());
            on_horizontal.insert(edge.target());
        }
    }

    // Draw stations
    for (idx, name, (x, y), is_passing_loop) in &stations {
        let cx = (x - min_x + padding) * scale;
        let cy = (y - min_y + padding) * scale;

        if *is_passing_loop {
            let _ = writeln!(
                svg,
                r##"<circle cx="{cx}" cy="{cy}" r="5" fill="#666"/>"##
            );
        } else {
            let _ = writeln!(
                svg,
                r##"<circle cx="{cx}" cy="{cy}" r="8" fill="#e94560" stroke="white" stroke-width="1.5"/>"##
            );
            if on_horizontal.contains(idx) {
                // Rotate label -45° for stations on horizontal edges
                let _ = writeln!(
                    svg,
                    r##"<text x="{}" y="{}" font-size="14" fill="#ccc" font-family="sans-serif" transform="rotate(-45 {} {})">{name}</text>"##,
                    cx + 10.0, cy - 5.0, cx + 10.0, cy - 5.0
                );
            } else {
                let _ = writeln!(
                    svg,
                    r##"<text x="{}" y="{}" font-size="14" fill="#ccc" font-family="sans-serif">{name}</text>"##,
                    cx + 10.0,
                    cy + 5.0
                );
            }
        }
    }

    let _ = writeln!(svg, "</svg>");
    svg
}

fn main() {
    let content =
        fs::read_to_string("timetable.json").expect("Failed to read timetable.json");

    let data = parse_nimby_json(&content).expect("Failed to parse NIMBY JSON");
    println!(
        "Stations: {}, Lines: {}",
        data.stations.len(),
        data.lines.len()
    );

    // Start with a small set for MIP testing
    let allowed_codes = [
        "RE10", "R12", "R13", "R14", "R21", "R22", "RE30", "RE20", "R31", "L1", "L2", "R15",
    ];
    let selected: Vec<String> = data
        .lines
        .iter()
        .filter(|l| {
            let through_oslo = l.stops.iter().any(|stop| {
                data.stations
                    .get(&stop.station_id)
                    .is_some_and(|s| s.name.to_lowercase().contains("oslo"))
            });
            through_oslo && allowed_codes.contains(&l.code.as_str())
        })
        .map(|l| l.id.clone())
        .collect();

    println!("Selected {} lines", selected.len());

    let mut graph = RailwayGraph::default();
    let config = NimbyImportConfig {
        create_infrastructure: true,
        selected_line_ids: selected,
        handedness: TrackHandedness::RightHand,
        station_spacing: 60.0,
        ..Default::default()
    };

    let lines = import_nimby_lines(&data, &config, &mut graph, 0, None)
        .expect("Failed to import");

    println!(
        "Imported {} lines, {} nodes, {} edges",
        lines.len(),
        graph.graph.node_count(),
        graph.graph.edge_count()
    );

    // Generate SVG
    let svg = generate_svg(&graph);
    fs::write("layout_output.svg", &svg).expect("Failed to write SVG");
    println!("Wrote layout_output.svg ({} bytes)", svg.len());
}
