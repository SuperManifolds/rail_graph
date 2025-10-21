use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::detect_line_conflicts;
use nimby_graph::import::csv::{analyze_csv, parse_csv_with_mapping};
use nimby_graph::models::{RailwayGraph, Stations};
use std::fs;

fn main() {
    // Load all CSV files from test-data directory
    let paths = fs::read_dir("test-data")
        .expect("Failed to read test-data directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().and_then(|s| s.to_str()) == Some("csv")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    println!("Found {} CSV files", paths.len());

    let mut graph = RailwayGraph::new();
    let mut all_lines = Vec::new();

    for path in &paths {
        let filename = path.file_name().expect("path should have filename");
        println!("Loading {}...", filename.to_string_lossy());
        let csv_content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", filename.to_string_lossy()));

        let config = analyze_csv(&csv_content)
            .unwrap_or_else(|| panic!("Failed to analyze {}", filename.to_string_lossy()));

        let lines = parse_csv_with_mapping(&csv_content, &config, &mut graph, all_lines.len());
        all_lines.extend(lines);
    }

    println!("\nTotal lines loaded: {}", all_lines.len());
    println!("Total stations: {}", graph.get_all_stations_ordered().len());

    let journeys = TrainJourney::generate_journeys(&all_lines, &graph, None);
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    println!("Generated {} journeys", journeys_vec.len());

    // Run conflict detection (timing happens inside the function)
    let (conflicts, crossings) = detect_line_conflicts(&journeys_vec, &graph);

    println!("\nResults:");
    println!("  Conflicts: {}", conflicts.len());
    println!("  Crossings: {}", crossings.len());
}
