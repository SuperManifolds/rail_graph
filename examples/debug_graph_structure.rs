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

    // List all nodes by type and degree
    println!("=== All nodes by type ===");
    let mut stations = Vec::new();
    let mut junctions = Vec::new();

    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        let degree = project.graph.graph.neighbors(node_idx).count();

        if node.is_junction() {
            junctions.push((node.display_name(), degree));
        } else {
            stations.push((node.display_name(), degree));
        }
    }

    println!("Stations ({}):", stations.len());
    for (name, degree) in &stations {
        println!("  {name} (degree {degree})");
    }
    println!();

    println!("Junctions ({}):", junctions.len());
    for (name, degree) in &junctions {
        println!("  {name} (degree {degree})");
    }
    println!();

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

    // Show junction positions
    println!("=== Junction positions after autolayout ===");
    for node_idx in project.graph.graph.node_indices() {
        let node = &project.graph.graph[node_idx];
        if node.is_junction() {
            if let Some(pos) = node.position() {
                println!("{} at ({:.1}, {:.1})", node.display_name(), pos.0, pos.1);

                // Show its neighbors
                let neighbors: Vec<_> = project.graph.graph
                    .neighbors(node_idx)
                    .collect();

                for neighbor in neighbors {
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
                        } else {
                            &format!("{direction:.0}°")
                        };

                        println!("  -> {} at ({:.1}, {:.1}), direction: {}",
                            neighbor_node.display_name(), neighbor_pos.0, neighbor_pos.1, direction_name);
                    }
                }
                println!();
            }
        }
    }
}
