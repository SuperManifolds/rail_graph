#![allow(clippy::cast_possible_truncation)]

use nimby_graph::storage::deserialize_project_from_bytes;
use nimby_graph::import::{parse_nimby_json, import_nimby_lines, NimbyImportConfig, nimby::NimbyImportData};
use nimby_graph::models::{RailwayGraph, TrackHandedness};
use std::collections::HashMap;
use std::fs;

fn find_oslo_lines(data: &NimbyImportData) -> Vec<String> {
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

fn run_nimby_import() {
    let content = fs::read_to_string("timetable.json")
        .expect("Failed to read timetable.json");
    let data = parse_nimby_json(&content).expect("Failed to parse NIMBY JSON");

    let oslo_lines = find_oslo_lines(&data);
    println!("Importing {} lines through Oslo", oslo_lines.len());

    let mut graph = RailwayGraph::default();
    let config = NimbyImportConfig {
        create_infrastructure: true,
        selected_line_ids: oslo_lines,
        handedness: TrackHandedness::RightHand,
        station_spacing: 100.0,
        ..Default::default()
    };

    let _ = import_nimby_lines(&data, &config, &mut graph, 0, None)
        .expect("Failed to import");

    // Build edge usage map to test find_heaviest_path
    let mut edge_usage: HashMap<petgraph::stable_graph::EdgeIndex, usize> = HashMap::new();
    for nimby_line in &data.lines {
        if nimby_line.stops.len() < 2 || nimby_line.code.starts_with("D-") {
            continue;
        }
        let station_id_to_node: HashMap<String, petgraph::stable_graph::NodeIndex> = graph.graph
            .node_indices()
            .filter_map(|idx| {
                graph.graph[idx].as_station()
                    .and_then(|s| s.external_id.clone())
                    .map(|ext_id| (ext_id, idx))
            })
            .collect();

        let valid_stops: Vec<_> = nimby_line.stops.iter()
            .filter(|s| s.station_id != "0x0")
            .filter_map(|s| station_id_to_node.get(&s.station_id).copied())
            .collect();

        for window in valid_stops.windows(2) {
            if let Some(edge) = graph.graph.find_edge(window[0], window[1])
                .or_else(|| graph.graph.find_edge(window[1], window[0]))
            {
                *edge_usage.entry(edge).or_insert(0) += 1;
            }
        }
    }

    // Show highest usage edges
    let mut edge_list: Vec<_> = edge_usage.iter().collect();
    edge_list.sort_by_key(|(_, &count)| std::cmp::Reverse(count));
    println!("\nTop 20 most-used edges:");
    for (edge, count) in edge_list.iter().take(20) {
        if let Some((from, to)) = graph.graph.edge_endpoints(**edge) {
            let from_name = graph.graph[from].display_name();
            let to_name = graph.graph[to].display_name();
            println!("  {from_name} <-> {to_name}: {count} lines");
        }
    }

    // Show what heaviest path returns
    let heaviest_path = graph.find_heaviest_path(&edge_usage);
    println!("\nHeaviest path ({} nodes):", heaviest_path.len());
    for idx in &heaviest_path {
        println!("  - {}", graph.graph[*idx].display_name());
    }

    // Show what the spine would be with find_longest_path
    let longest_path = graph.find_longest_path();
    println!("\nLongest path ({} nodes):", longest_path.len());
    for idx in longest_path.iter().take(30) {
        println!("  - {}", graph.graph[*idx].display_name());
    }
    if longest_path.len() > 30 {
        println!("  ... ({} more)", longest_path.len() - 30);
    }

    println!("Graph has {} nodes and {} edges", graph.graph.node_count(), graph.graph.edge_count());

    // Show geographic coordinates for key stations
    println!("\n=== Geographic Coordinates ===");
    let key_stations = ["Oslo S", "Ski", "Lillestrøm", "Drammen", "Roa", "Heggedal", "Gjøvik", "Nationaltheatret"];
    for station_name in key_stations {
        if let Some((station_id, station)) = data.stations.iter().find(|(_, s)| s.name == station_name) {
            println!("{}: lonlat=({:.4}, {:.4}), id={}", station_name, station.lonlat.0, station.lonlat.1, station_id);
        }
    }

    // Calculate and show directions from Oslo S
    if let Some((oslo_id, oslo)) = data.stations.iter().find(|(_, s)| s.name == "Oslo S") {
        println!("\n=== Directions from Oslo S ===");
        for other_name in ["Ski", "Lillestrøm", "Drammen", "Roa", "Heggedal", "Gjøvik"] {
            if let Some((_, other)) = data.stations.iter().find(|(_, s)| s.name == other_name) {
                let dx = other.lonlat.0 - oslo.lonlat.0;  // longitude difference (east is positive)
                let dy = oslo.lonlat.1 - other.lonlat.1;  // latitude diff (north is negative Y on screen)
                let angle = dy.atan2(dx).to_degrees();
                let direction = if angle > -22.5 && angle <= 22.5 { "E" }
                    else if angle > 22.5 && angle <= 67.5 { "SE" }
                    else if angle > 67.5 && angle <= 112.5 { "S" }
                    else if angle > 112.5 && angle <= 157.5 { "SW" }
                    else if angle > 157.5 || angle <= -157.5 { "W" }
                    else if angle > -157.5 && angle <= -112.5 { "NW" }
                    else if angle > -112.5 && angle <= -67.5 { "N" }
                    else { "NE" };
                println!("  {oslo_id} -> {other_name}: angle={angle:.0}° ({direction})");
            }
        }
    }

    // Show positions grouped by X
    let mut by_x: HashMap<i32, Vec<(String, i32, String)>> = HashMap::new();

    for node_idx in graph.graph.node_indices() {
        let node = &graph.graph[node_idx];
        if let Some(station) = node.as_station() {
            if let Some((x, y)) = node.position() {
                let x_grid = (x / 10.0).round() as i32;
                let y_grid = (y / 10.0).round() as i32;
                let ext_id = station.external_id.as_deref().unwrap_or("?");
                by_x.entry(x_grid)
                    .or_default()
                    .push((node.display_name().clone(), y_grid, ext_id.to_string()));
            }
        }
    }

    let mut x_coords: Vec<_> = by_x.keys().copied().collect();
    x_coords.sort_unstable();

    println!("\nStations grouped by X coordinate:");
    for x in x_coords {
        let Some(stations_ref) = by_x.get(&x) else { continue };
        let mut stations = stations_ref.clone();
        stations.sort_by_key(|(_, y, _)| *y);

        println!("\nX = {x}:");
        for (name, y, _) in stations {
            println!("  y={y:3}: {name}");
        }
    }
}

fn main() {
    // First, run the NIMBY import to see what the algorithm produces
    println!("=== NIMBY Import Algorithm Output ===\n");
    run_nimby_import();

    println!("\n\n=== Manual Layout Reference ===\n");

    let bytes = fs::read("manual.rgproject").expect("Failed to read manual.rgproject");
    let project = deserialize_project_from_bytes(&bytes).expect("Failed to deserialize project");

    println!("Project: {}", project.metadata.name);
    println!("Nodes: {}", project.graph.graph.node_count());
    println!("Edges: {}", project.graph.graph.edge_count());
    println!();

    // Collect all stations with positions, grouped by X coordinate
    let mut by_x: HashMap<i32, Vec<(String, i32, String)>> = HashMap::new();

    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        if let Some(station) = node.as_station() {
            if let Some((x, y)) = node.position() {
                let x_grid = (x / 10.0).round() as i32;
                let y_grid = (y / 10.0).round() as i32;
                let ext_id = station.external_id.as_deref().unwrap_or("?");
                by_x.entry(x_grid)
                    .or_default()
                    .push((node.display_name().clone(), y_grid, ext_id.to_string()));
            }
        }
    }

    // Sort and print columns (X coordinates)
    let mut x_coords: Vec<_> = by_x.keys().copied().collect();
    x_coords.sort_unstable();

    println!("=== Stations grouped by X coordinate (columns) ===");
    for x in x_coords {
        let Some(stations_ref) = by_x.get(&x) else { continue };
        let mut stations = stations_ref.clone();
        stations.sort_by_key(|(_, y, _)| *y);

        println!("\nX = {x} (column):");
        for (name, y, ext_id) in stations {
            println!("  y={y:3}: {name} ({ext_id})");
        }
    }

    // Also show the graph structure - what's connected to what
    println!("\n\n=== Graph connections ===");
    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        let name = node.display_name();

        let neighbors: Vec<_> = project.graph.graph
            .neighbors(node_idx)
            .map(|n| project.graph.graph[n].display_name().clone())
            .collect();

        if !neighbors.is_empty() {
            println!("{name} -> {neighbors:?}");
        }
    }
}
