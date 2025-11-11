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
    println!();

    // Find all nodes with degree > 2 (potential branches)
    println!("=== Branching nodes (degree > 2) ===");
    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        let degree = project.graph.graph.neighbors(node_idx).count();
        if degree > 2 {
            println!("{} (degree {}): neighbors:", node.display_name(), degree);
            for neighbor in project.graph.graph.neighbors(node_idx) {
                let neighbor_node = &project.graph.graph[neighbor];
                println!("  - {}", neighbor_node.display_name());
            }
            println!();
        }
    }

    // Clear all positions and run autolayout
    println!("=== Running autolayout ===");
    let node_indices: Vec<_> = project.graph.graph.node_indices().collect();
    for node_idx in node_indices {
        let node = &mut project.graph.graph[node_idx];
        node.set_position(None);
    }

    let settings = ProjectSettings::default();
    auto_layout::apply_layout(&mut project.graph, 1000.0, &settings);
    println!();

    // Check where branches went after autolayout
    println!("=== Branch directions after autolayout ===");
    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        let degree = project.graph.graph.neighbors(node_idx).count();
        if degree > 2 {
            if let Some(pos) = node.position() {
                println!("{} at ({:.1}, {:.1}):", node.display_name(), pos.0, pos.1);

                for neighbor in project.graph.graph.neighbors(node_idx) {
                    let neighbor_node = &project.graph.graph[neighbor];
                    #[allow(clippy::excessive_nesting)]
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
                        } else if direction > 0.0 && direction < 60.0 {
                            "SOUTHEAST"
                        } else if direction < 0.0 && direction > -60.0 {
                            "NORTHEAST"
                        } else if direction > 120.0 && direction < 180.0 {
                            "SOUTHWEST"
                        } else if direction < -120.0 && direction > -180.0 {
                            "NORTHWEST"
                        } else {
                            "OTHER"
                        };

                        println!("  - {} at ({:.1}, {:.1}), dx={:.1}, dy={:.1}, direction={:.1}° ({})",
                            neighbor_node.display_name(), neighbor_pos.0, neighbor_pos.1, dx, dy, direction, direction_name);
                    }
                }
                println!();
            }
        }
    }

    // Count directions per branch
    println!("=== Summary of branch directions ===");
    let mut east_count = 0;
    let mut west_count = 0;
    let mut other_count = 0;

    for node_idx in project.graph.graph.node_indices() {
        let degree = project.graph.graph.neighbors(node_idx).count();
        if degree > 2 {
            if let Some(pos) = project.graph.graph[node_idx].position() {
                for neighbor in project.graph.graph.neighbors(node_idx) {
                    #[allow(clippy::excessive_nesting)]
                    if let Some(neighbor_pos) = project.graph.graph[neighbor].position() {
                        let dx = neighbor_pos.0 - pos.0;
                        let dy = neighbor_pos.1 - pos.1;
                        let direction = dy.atan2(dx).to_degrees();

                        if direction.abs() < 90.0 {
                            east_count += 1;
                        } else if (direction - 180.0).abs() < 90.0 || (direction + 180.0).abs() < 90.0 {
                            west_count += 1;
                        } else {
                            other_count += 1;
                        }
                    }
                }
            }
        }
    }

    println!("Total branches going EAST (hemisphere): {east_count}");
    println!("Total branches going WEST (hemisphere): {west_count}");
    println!("Total branches in OTHER directions: {other_count}");
}
