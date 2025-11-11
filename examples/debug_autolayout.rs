use nimby_graph::storage::deserialize_project_from_bytes;
use nimby_graph::components::infrastructure_canvas::auto_layout;
use nimby_graph::models::ProjectSettings;
use std::fs;

fn main() {
    // Read the glasgow.rgproject file
    let bytes = fs::read("glasgow.rgproject").expect("Failed to read glasgow.rgproject");

    // Deserialize the project
    let mut project = deserialize_project_from_bytes(&bytes).expect("Failed to deserialize project");

    println!("Project: {}", project.metadata.name);
    println!("Lines: {}", project.lines.len());
    println!("Stations: {}", project.graph.graph.node_count());
    println!();

    // Print all station names and their positions before autolayout
    println!("=== Stations before autolayout ===");
    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        let pos = node.position();
        println!("{}: pos={:?}", node.display_name(), pos);
    }
    println!();

    // Find Upper Tyndrum
    let upper_tyndrum_idx = project.graph.graph.node_indices()
        .find(|&idx| project.graph.graph[idx].display_name() == "Upper Tyndrum")
        .expect("Upper Tyndrum not found");

    println!("Found Upper Tyndrum at index: {upper_tyndrum_idx:?}");
    println!();

    // Clear all positions to force autolayout
    let node_indices: Vec<_> = project.graph.graph.node_indices().collect();
    for node_idx in node_indices {
        let node = &mut project.graph.graph[node_idx];
        node.set_position(None);
    }

    // Run autolayout with debug output
    println!("=== Running autolayout ===");
    let settings = ProjectSettings::default();
    auto_layout::apply_layout(&mut project.graph, 1000.0, &settings);
    println!();

    // Print positions after autolayout
    println!("=== Stations after autolayout ===");
    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        if let Some(pos) = node.position() {
            println!("{}: x={:.1}, y={:.1}", node.display_name(), pos.0, pos.1);
        }
    }
    println!();

    // Check Upper Tyndrum's position
    if let Some(pos) = project.graph.graph[upper_tyndrum_idx].position() {
        println!("Upper Tyndrum final position: x={:.1}, y={:.1}", pos.0, pos.1);

        // Find its neighbors to understand the branching
        let neighbors: Vec<_> = project.graph.graph
            .neighbors(upper_tyndrum_idx)
            .collect();

        println!("Upper Tyndrum has {} neighbors:", neighbors.len());
        for neighbor in neighbors {
            let neighbor_node = &project.graph.graph[neighbor];
            if let Some(neighbor_pos) = neighbor_node.position() {
                let dx = neighbor_pos.0 - pos.0;
                let dy = neighbor_pos.1 - pos.1;
                let direction = dy.atan2(dx).to_degrees();
                let direction_name = if direction.abs() < 30.0 {
                    "EAST"
                } else if (direction - 180.0).abs() < 30.0 || (direction + 180.0).abs() < 30.0 {
                    "WEST"
                } else if direction > 60.0 && direction < 120.0 {
                    "SOUTH"
                } else if direction < -60.0 && direction > -120.0 {
                    "NORTH"
                } else if direction > 0.0 {
                    "SOUTHEAST"
                } else if direction < 0.0 {
                    "NORTHEAST"
                } else {
                    "OTHER"
                };

                println!("  - {} at ({:.1}, {:.1}), dx={:.1}, dy={:.1}, direction={:.1}° ({})",
                    neighbor_node.display_name(), neighbor_pos.0, neighbor_pos.1, dx, dy, direction, direction_name);
            }
        }
    }
}
