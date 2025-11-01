use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::{detect_line_conflicts, SerializableConflictContext};
use nimby_graph::import::{Import, ImportMode, CsvImport};
use nimby_graph::models::RailwayGraph;

fn benchmark_conflict_detection(c: &mut Criterion) {
    // Load test data from R70.csv
    let csv_content = std::fs::read_to_string("test-data/R70.csv")
        .expect("Failed to read test-data/R70.csv");

    let config = CsvImport::analyze(&csv_content, None).expect("Failed to analyze CSV");
    let mut graph = RailwayGraph::new();
    let result = CsvImport::import_from_content(
        &csv_content,
        &config,
        ImportMode::CreateInfrastructure,
        &mut graph,
        0,
        &[],
        nimby_graph::models::TrackHandedness::RightHand,
    ).expect("Failed to import CSV");
    let lines = result.lines;

    let journeys = TrainJourney::generate_journeys(&lines, &graph, None);
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    // Build serializable context
    let station_indices = graph.graph.node_indices()
        .enumerate()
        .map(|(idx, node_idx)| (node_idx, idx))
        .collect();
    let context = SerializableConflictContext::from_graph(&graph, station_indices);

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
                black_box(&context),
            )
        });
    });

    // Benchmark the full pipeline (what happens on every change)
    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let journeys = TrainJourney::generate_journeys(black_box(&lines), black_box(&graph), None);
            let journeys_vec: Vec<_> = journeys.values().cloned().collect();

            let station_indices = graph.graph.node_indices()
                .enumerate()
                .map(|(idx, node_idx)| (node_idx, idx))
                .collect();
            let context = SerializableConflictContext::from_graph(&graph, station_indices);

            detect_line_conflicts(
                black_box(&journeys_vec),
                black_box(&context),
            )
        });
    });
}

criterion_group!(benches, benchmark_conflict_detection);
criterion_main!(benches);