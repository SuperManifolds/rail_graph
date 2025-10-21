use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::detect_line_conflicts;
use nimby_graph::import::csv::{analyze_csv, parse_csv_with_mapping};
use nimby_graph::models::RailwayGraph;

fn benchmark_conflict_detection(c: &mut Criterion) {
    // Load test data from R70.csv
    let csv_content = std::fs::read_to_string("test-data/R70.csv")
        .expect("Failed to read test-data/R70.csv");

    let config = analyze_csv(&csv_content).expect("Failed to analyze CSV");
    let mut graph = RailwayGraph::new();
    let lines = parse_csv_with_mapping(&csv_content, &config, &mut graph, 0);

    let journeys = TrainJourney::generate_journeys(&lines, &graph, None);
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    // Benchmark journey generation
    c.bench_function("generate_journeys", |b| {
        b.iter(|| {
            TrainJourney::generate_journeys(black_box(&lines), black_box(&graph), None)
        });
    });

    // Benchmark conflict detection
    c.bench_function("conflict_detection", |b| {
        b.iter(|| {
            detect_line_conflicts(
                black_box(&journeys_vec),
                black_box(&graph),
            )
        });
    });

    // Benchmark the full pipeline (what happens on every change)
    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let journeys = TrainJourney::generate_journeys(black_box(&lines), black_box(&graph), None);
            let journeys_vec: Vec<_> = journeys.values().cloned().collect();
            detect_line_conflicts(
                black_box(&journeys_vec),
                black_box(&graph),
            )
        });
    });
}

criterion_group!(benches, benchmark_conflict_detection);
criterion_main!(benches);