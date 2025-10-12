use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::detect_line_conflicts;
use nimby_graph::data::parse_csv_data;

fn main() {
    // Load real data from lines.csv
    let (lines, graph) = parse_csv_data();
    let journeys = TrainJourney::generate_journeys(&lines, &graph, None);
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    println!("Loaded {} journeys", journeys_vec.len());

    // Run conflict detection (timing happens inside the function)
    let (conflicts, crossings) = detect_line_conflicts(&journeys_vec, &graph);

    println!("\nResults:");
    println!("  Conflicts: {}", conflicts.len());
    println!("  Crossings: {}", crossings.len());
}
