use nimby_graph::import::{parse_nimby_json, import_nimby_lines, NimbyImportConfig};
use nimby_graph::models::{RailwayGraph, TrackHandedness};
use petgraph::Direction;
use std::collections::{HashSet, VecDeque};
use std::fs;

fn main() {
    let content = fs::read_to_string("timetable.json")
        .expect("Failed to read timetable.json");

    let data = parse_nimby_json(&content).expect("Failed to parse NIMBY JSON");

    println!("Parsed NIMBY data:");
    println!("  Company: {}", data.company_name);
    println!("  Stations: {}", data.stations.len());
    println!("  Lines: {}", data.lines.len());
    println!();

    // Find R14 line
    let r14_id = data.lines.iter()
        .find(|l| l.code == "R14")
        .map(|l| l.id.clone())
        .expect("R14 line not found");

    println!("Importing R14 only...");

    let mut graph = RailwayGraph::default();
    let config = NimbyImportConfig {
        create_infrastructure: true,
        selected_line_ids: vec![r14_id],
        handedness: TrackHandedness::RightHand,
        station_spacing: 100.0,
        ..Default::default()
    };

    let _lines = import_nimby_lines(&data, &config, &mut graph, 0, None)
        .expect("Failed to import NIMBY lines");

    let nodes_after_first = graph.graph.node_count();
    let edges_after_first = graph.graph.edge_count();

    println!("After first import: {nodes_after_first} nodes and {edges_after_first} edges");

    // Import the same line again to test deduplication
    println!("Importing R14 again (should not create duplicate loops)...");
    let _lines2 = import_nimby_lines(&data, &config, &mut graph, 0, None)
        .expect("Failed to import NIMBY lines second time");

    let nodes_after_second = graph.graph.node_count();
    let edges_after_second = graph.graph.edge_count();

    println!("After second import: {nodes_after_second} nodes and {edges_after_second} edges");

    // Check for duplicates
    println!();
    println!("=== Duplicate Check ===");
    if nodes_after_first == nodes_after_second && edges_after_first == edges_after_second {
        println!("  SUCCESS: No duplicate nodes or edges created");
    } else {
        println!("  FAILED: Duplicate import created {} extra nodes and {} extra edges",
            nodes_after_second - nodes_after_first,
            edges_after_second - edges_after_first);
    }
    println!();

    // Check for passing loops
    println!("=== Passing Loops ===");
    let mut passing_loop_count = 0;
    for idx in graph.graph.node_indices() {
        if let Some(station) = graph.graph[idx].as_station() {
            if station.passing_loop {
                passing_loop_count += 1;
                println!("  Found: {} (passing_loop=true)", station.name);
            }
        }
    }

    if passing_loop_count == 0 {
        println!("  No passing loops detected!");
    } else {
        println!();
        println!("Total passing loops: {passing_loop_count}");
    }

    // List all stations
    println!();
    println!("=== All Stations ===");
    for idx in graph.graph.node_indices() {
        if let Some(station) = graph.graph[idx].as_station() {
            let loop_marker = if station.passing_loop { " [LOOP]" } else { "" };
            println!("  {}{}", station.name, loop_marker);
        }
    }

    // Check for depots (should not exist)
    println!();
    println!("=== Depot Check ===");
    let depot_count = graph.graph.node_indices()
        .filter(|&idx| graph.graph[idx].display_name().contains("[DEP]"))
        .count();
    if depot_count == 0 {
        println!("  SUCCESS: No depots in graph");
    } else {
        println!("  FAILED: Found {depot_count} depots in graph");
    }

    // Check connectivity
    println!();
    println!("=== Connectivity ===");
    check_connectivity(&graph);
}

fn check_connectivity(graph: &RailwayGraph) {
    // Check for orphan nodes
    let mut orphan_count = 0;
    for idx in graph.graph.node_indices() {
        let neighbors: usize = graph.graph.neighbors_directed(idx, Direction::Outgoing).count()
            + graph.graph.neighbors_directed(idx, Direction::Incoming).count();
        if neighbors == 0 {
            orphan_count += 1;
            println!("  ORPHAN: {}", graph.graph[idx].display_name());
        }
    }
    if orphan_count == 0 {
        println!("  All nodes connected (no orphans)");
    }

    // Check if graph is connected using BFS
    let start = graph.graph.node_indices().next();
    if let Some(start) = start {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            for neighbor in graph.graph.neighbors_directed(current, Direction::Outgoing)
                .chain(graph.graph.neighbors_directed(current, Direction::Incoming))
            {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }

        let total_nodes = graph.graph.node_count();
        if visited.len() == total_nodes {
            println!("  Graph is fully connected ({total_nodes} nodes reachable)");
        } else {
            println!("  WARNING: Graph is disconnected! {} of {} nodes reachable",
                visited.len(), total_nodes);
        }
    }
}
