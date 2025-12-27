use nimby_graph::import::{parse_nimby_json, import_nimby_lines, NimbyImportConfig};
use nimby_graph::models::{RailwayGraph, TrackHandedness};
use petgraph::Direction;
use petgraph::graph::NodeIndex;
use std::fs;

fn print_summary(data: &nimby_graph::import::nimby::NimbyImportData) {
    println!("Parsed NIMBY data:");
    println!("  Company: {}", data.company_name);
    println!("  Stations: {}", data.stations.len());
    println!("  Lines: {}", data.lines.len());
    println!();
}

fn find_oslo_lines(data: &nimby_graph::import::nimby::NimbyImportData) -> Vec<String> {
    let allowed_codes = ["RE10", "R12", "R13", "R14", "R21", "R22", "RE30", "RE20", "R31", "L1", "L2", "R15"];
    data.lines.iter()
        .filter(|line| {
            let through_oslo = line.stops.iter().any(|stop| {
                data.stations.get(&stop.station_id)
                    .is_some_and(|s| s.name.to_lowercase().contains("oslo"))
            });
            through_oslo && allowed_codes.contains(&line.code.as_str())
        })
        .map(|line| line.id.clone())
        .collect()
}

fn print_lysaker_sandvika_analysis(data: &nimby_graph::import::nimby::NimbyImportData, oslo_lines: &[String]) {
    println!("=== Lysaker-Sandvika Analysis ===");

    // Find Lysaker and Sandvika station IDs
    let lysaker_id = data.stations.iter()
        .find(|(_, s)| s.name.to_lowercase() == "lysaker")
        .map(|(id, _)| id.clone());
    let sandvika_id = data.stations.iter()
        .find(|(_, s)| s.name.to_lowercase() == "sandvika")
        .map(|(id, _)| id.clone());

    let (Some(lysaker_id), Some(sandvika_id)) = (lysaker_id, sandvika_id) else {
        println!("  Could not find Lysaker or Sandvika stations");
        return;
    };

    println!("  Lysaker: {lysaker_id}");
    println!("  Sandvika: {sandvika_id}");
    println!();

    // Find all lines that have both stations and analyze the path
    for line in &data.lines {
        if !oslo_lines.contains(&line.id) {
            continue;
        }

        let stations: Vec<_> = line.stops.iter()
            .enumerate()
            .filter(|(_, s)| s.station_id != "0x0")
            .collect();

        // Find Lysaker and Sandvika positions
        let lysaker_pos = stations.iter().position(|(_, s)| s.station_id == lysaker_id);
        let sandvika_pos = stations.iter().position(|(_, s)| s.station_id == sandvika_id);

        if let (Some(lpos), Some(spos)) = (lysaker_pos, sandvika_pos) {
            let (from_pos, to_pos) = if lpos < spos { (lpos, spos) } else { (spos, lpos) };
            let intermediates: Vec<_> = stations[from_pos + 1..to_pos]
                .iter()
                .filter_map(|(_, s)| data.stations.get(&s.station_id).map(|st| st.name.as_str()))
                .collect();

            // Calculate distance
            let from_idx = stations[from_pos].0;
            let to_idx = stations[to_pos].0;
            let distance: f64 = line.stops[from_idx + 1..=to_idx]
                .iter()
                .map(|s| s.leg_distance)
                .sum();

            println!("  {} ({}): Lysaker -> Sandvika", line.name, line.code);
            println!("    Distance: {distance:.0}m");
            println!("    Intermediates: {intermediates:?}");
            println!();
        }
    }
}

fn print_oslo_sinsen_adjacencies(data: &nimby_graph::import::nimby::NimbyImportData, oslo_lines: &[String]) {
    println!("=== Lines with Oslo S -> Sinsen(0x2000000090001) adjacency ===");
    for line in &data.lines {
        if !oslo_lines.contains(&line.id) {
            continue;
        }
        let stops: Vec<_> = line.stops.iter()
            .filter(|s| s.station_id != "0x0")
            .collect();
        for window in stops.windows(2) {
            let a_id = &window[0].station_id;
            let b_id = &window[1].station_id;
            if (a_id == "0x2000000010001" && b_id == "0x2000000090001")
                || (a_id == "0x2000000090001" && b_id == "0x2000000010001")
            {
                let a_name = data.stations.get(a_id).map_or("?", |s| s.name.as_str());
                let b_name = data.stations.get(b_id).map_or("?", |s| s.name.as_str());
                println!("  Line {} ({}): {a_name} ({a_id}) -> {b_name} ({b_id})",
                    line.name, line.code);
            }
        }
    }
    println!();
}

fn analyze_oslo_connections(graph: &RailwayGraph) -> Option<NodeIndex> {
    let oslo_nodes: Vec<_> = graph.graph.node_indices()
        .filter(|&idx| graph.graph[idx].display_name().to_lowercase().contains("oslo"))
        .collect();

    println!("Found {} Oslo-related stations:", oslo_nodes.len());
    for &idx in &oslo_nodes {
        let node = &graph.graph[idx];
        let out_count = graph.graph.neighbors_directed(idx, Direction::Outgoing).count();
        let in_count = graph.graph.neighbors_directed(idx, Direction::Incoming).count();
        println!("  - {} (out: {out_count}, in: {in_count})", node.display_name());
    }
    println!();

    oslo_nodes.first().copied()
}

fn check_sinsen_metro_connection(graph: &RailwayGraph, oslo_idx: NodeIndex) {
    let sinsen_metro_id = "0x2000004d10001";
    let sinsen_metro_neighbor = graph.graph.neighbors_directed(oslo_idx, Direction::Outgoing)
        .chain(graph.graph.neighbors_directed(oslo_idx, Direction::Incoming))
        .any(|idx| {
            graph.graph[idx].as_station()
                .and_then(|s| s.external_id.as_deref())
                == Some(sinsen_metro_id)
        });

    println!("=== Analysis ===");
    println!();
    if sinsen_metro_neighbor {
        println!("BUG FOUND: Oslo S is directly connected to Sinsen metro (0x2000004d10001)!");
        println!("Express trains should route through intermediate stations, not create direct tracks.");
    } else {
        println!("SUCCESS: Sinsen metro (0x2000004d10001) is NOT a direct neighbor of Oslo S.");
        println!("Express trains correctly route through intermediate stations.");
    }
}

fn check_orphan_nodes(graph: &RailwayGraph) {
    println!();
    println!("=== ORPHAN NODES (no connections) ===");
    let mut orphan_count = 0;
    for idx in graph.graph.node_indices() {
        let out = graph.graph.neighbors_directed(idx, Direction::Outgoing).count();
        let inc = graph.graph.neighbors_directed(idx, Direction::Incoming).count();
        if out == 0 && inc == 0 {
            let name = graph.graph[idx].display_name();
            if let Some(station) = graph.graph[idx].as_station() {
                println!("  ORPHAN: {} (external_id={:?})", name, station.external_id);
                orphan_count += 1;
            }
        }
    }
    if orphan_count == 0 {
        println!("  None - all stations are connected!");
    } else {
        println!();
        println!("Found {orphan_count} orphan nodes!");
    }
}

fn check_specific_stations(graph: &RailwayGraph) {
    println!();
    println!("=== Checking for Gjøvik and Dal ===");
    for idx in graph.graph.node_indices() {
        let name = graph.graph[idx].display_name();
        if name.to_lowercase().contains("gjøvik") || name.to_lowercase().contains("dal") {
            let out = graph.graph.neighbors_directed(idx, Direction::Outgoing).count();
            let inc = graph.graph.neighbors_directed(idx, Direction::Incoming).count();
            if let Some(station) = graph.graph[idx].as_station() {
                println!("  Found: {} (external_id={:?}, out={out}, in={inc})",
                    name, station.external_id);
            }
        }
    }
}

fn check_lysaker_sandvika_edges(graph: &RailwayGraph) {
    println!();
    println!("=== Lysaker-Sandvika Graph Edges ===");

    // Find Lysaker and Sandvika nodes
    let lysaker = graph.graph.node_indices()
        .find(|&idx| graph.graph[idx].display_name().to_lowercase() == "lysaker");
    let sandvika = graph.graph.node_indices()
        .find(|&idx| graph.graph[idx].display_name().to_lowercase() == "sandvika");

    let (Some(lysaker_idx), Some(sandvika_idx)) = (lysaker, sandvika) else {
        println!("  Could not find Lysaker or Sandvika in graph");
        return;
    };

    // Check direct edge
    let direct_edge = graph.graph.find_edge(lysaker_idx, sandvika_idx)
        .or_else(|| graph.graph.find_edge(sandvika_idx, lysaker_idx));

    if let Some(edge_idx) = direct_edge {
        if let Some(segment) = graph.graph.edge_weight(edge_idx) {
            println!("  Direct edge Lysaker <-> Sandvika: distance={:?}m", segment.distance);
        }
    } else {
        println!("  NO direct edge between Lysaker and Sandvika");
    }

    // Check Lysaker's neighbors
    println!("  Lysaker neighbors:");
    for neighbor in graph.graph.neighbors_directed(lysaker_idx, Direction::Outgoing)
        .chain(graph.graph.neighbors_directed(lysaker_idx, Direction::Incoming))
    {
        println!("    - {}", graph.graph[neighbor].display_name());
    }
}

fn main() {
    let content = fs::read_to_string("timetable.json")
        .expect("Failed to read timetable.json - make sure it's in the current directory");

    let data = parse_nimby_json(&content).expect("Failed to parse NIMBY JSON");
    print_summary(&data);

    let oslo_lines = find_oslo_lines(&data);
    println!("Found {} lines through Oslo:", oslo_lines.len());
    for id in &oslo_lines {
        if let Some(line) = data.lines.iter().find(|l| l.id == *id) {
            println!("  - {} ({})", line.name, line.code);
        }
    }
    println!();

    print_lysaker_sandvika_analysis(&data, &oslo_lines);
    print_oslo_sinsen_adjacencies(&data, &oslo_lines);

    let mut graph = RailwayGraph::default();
    let config = NimbyImportConfig {
        create_infrastructure: true,
        selected_line_ids: oslo_lines.clone(),
        handedness: TrackHandedness::RightHand,
        station_spacing: 100.0,
    };

    println!("Importing {} lines through Oslo...", oslo_lines.len());
    let lines = import_nimby_lines(&data, &config, &mut graph, 0)
        .expect("Failed to import NIMBY lines");

    println!("Imported {} lines", lines.len());
    println!("Graph has {} nodes and {} edges",
        graph.graph.node_count(),
        graph.graph.edge_count());
    println!();

    // Find Sinsen stations
    let sinsen_nodes: Vec<_> = graph.graph.node_indices()
        .filter(|&idx| graph.graph[idx].display_name().to_lowercase().contains("sinsen"))
        .collect();
    println!("Found {} Sinsen stations:", sinsen_nodes.len());
    for &idx in &sinsen_nodes {
        let node = &graph.graph[idx];
        if let Some(station) = node.as_station() {
            println!("  - {} (external_id={:?})", node.display_name(), station.external_id);
        }
    }
    println!();

    let Some(oslo_idx) = analyze_oslo_connections(&graph) else {
        println!("ERROR: Could not find Oslo station!");
        return;
    };

    let oslo = &graph.graph[oslo_idx];
    println!("Analyzing connections for: {}", oslo.display_name());
    println!();

    if let Some(oslo_station) = graph.graph[oslo_idx].as_station() {
        println!("Oslo S external_id: {:?}", oslo_station.external_id);
    }

    // Get all connections
    let outgoing: Vec<_> = graph.graph.neighbors_directed(oslo_idx, Direction::Outgoing)
        .map(|idx| {
            let name = graph.graph[idx].display_name().clone();
            if let Some(station) = graph.graph[idx].as_station() {
                println!("  Neighbor {name}: external_id={:?}", station.external_id);
            }
            name
        })
        .collect();
    let incoming: Vec<_> = graph.graph.neighbors_directed(oslo_idx, Direction::Incoming)
        .map(|idx| {
            let name = graph.graph[idx].display_name().clone();
            if let Some(station) = graph.graph[idx].as_station() {
                if !outgoing.contains(&name) {
                    println!("  Neighbor (incoming only) {name}: external_id={:?}", station.external_id);
                }
            }
            name
        })
        .collect();

    let mut neighbors: Vec<_> = outgoing.iter().chain(incoming.iter()).cloned().collect();
    neighbors.sort();
    neighbors.dedup();

    println!("Oslo has {} connections:", neighbors.len());
    for name in &neighbors {
        println!("  - {name}");
    }
    println!();

    check_sinsen_metro_connection(&graph, oslo_idx);
    check_orphan_nodes(&graph);
    check_specific_stations(&graph);
    check_lysaker_sandvika_edges(&graph);
}
