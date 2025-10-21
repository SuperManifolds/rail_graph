use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::detect_line_conflicts;
use nimby_graph::import::csv::{analyze_csv, parse_csv_with_mapping};
use nimby_graph::models::RailwayGraph;

fn main() {
    // Load test data from R70.csv
    let csv_content = std::fs::read_to_string("test-data/R70.csv")
        .expect("Failed to read test-data/R70.csv");

    let config = analyze_csv(&csv_content).expect("Failed to analyze CSV");
    let mut graph = RailwayGraph::new();
    let lines = parse_csv_with_mapping(&csv_content, &config, &mut graph, 0);

    let journeys = TrainJourney::generate_journeys(&lines, &graph, None);
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    println!("Loaded {} journeys", journeys_vec.len());

    // Run conflict detection (timing happens inside the function)
    let (conflicts, crossings) = detect_line_conflicts(&journeys_vec, &graph);

    println!("\nResults:");
    println!("  Conflicts: {}", conflicts.len());
    println!("  Crossings: {}", crossings.len());
}
