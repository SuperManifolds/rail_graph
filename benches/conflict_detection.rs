use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nimby_graph::train_journey::TrainJourney;
use nimby_graph::conflict::detect_line_conflicts;
use nimby_graph::data::parse_csv_data;

fn benchmark_conflict_detection(c: &mut Criterion) {
    // Load real data from lines.csv
    let (lines, graph) = parse_csv_data();
    let journeys = TrainJourney::generate_journeys(&lines, &graph);

    // Benchmark journey generation
    c.bench_function("generate_journeys", |b| {
        b.iter(|| {
            TrainJourney::generate_journeys(black_box(&lines), black_box(&graph))
        });
    });

    // Benchmark conflict detection
    c.bench_function("conflict_detection", |b| {
        b.iter(|| {
            detect_line_conflicts(
                black_box(&journeys),
                black_box(&graph),
            )
        });
    });

    // Benchmark the full pipeline (what happens on every change)
    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let journeys = TrainJourney::generate_journeys(black_box(&lines), black_box(&graph));
            detect_line_conflicts(
                black_box(&journeys),
                black_box(&graph),
            )
        });
    });
}

criterion_group!(benches, benchmark_conflict_detection);
criterion_main!(benches);