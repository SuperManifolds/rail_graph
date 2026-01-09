use nimby_graph::storage::deserialize_project_from_bytes;
use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::{detect_line_conflicts, SerializableConflictContext};
use std::fs;
use std::time::Instant;

fn main() {
    // Load perf.rgproject
    let bytes = fs::read("perf.rgproject")
        .expect("Failed to read perf.rgproject");

    let project = deserialize_project_from_bytes(&bytes)
        .expect("Failed to deserialize project");

    println!("Project: {}", project.metadata.name);
    println!("Nodes: {}", project.graph.graph.node_count());
    println!("Edges: {}", project.graph.graph.edge_count());
    println!("Lines: {}", project.lines.len());

    // Generate journeys from lines
    let start = Instant::now();
    let journeys = TrainJourney::generate_journeys(&project.lines, &project.graph, None);
    let journey_time = start.elapsed();

    let journeys_vec: Vec<_> = journeys.values().cloned().collect();
    println!("Generated {} journeys in {:?}", journeys_vec.len(), journey_time);

    // Build context
    let start = Instant::now();
    let station_indices = project.graph.graph.node_indices()
        .enumerate()
        .map(|(idx, node_idx)| (node_idx, idx))
        .collect();

    let context = SerializableConflictContext::from_graph(
        &project.graph,
        station_indices,
        project.settings.station_margin,
        project.settings.minimum_separation,
        project.settings.ignore_same_direction_platform_conflicts,
    );
    let context_time = start.elapsed();
    println!("Built context in {context_time:?}");

    // Run conflict detection (timing printed inside)
    println!("\n=== Running Conflict Detection ===\n");
    let (conflicts, crossings) = detect_line_conflicts(&journeys_vec, &context);

    // Summary
    println!("\n=== Summary ===");
    println!("Conflicts: {}", conflicts.len());
    println!("Crossings: {}", crossings.len());
}
