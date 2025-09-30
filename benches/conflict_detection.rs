use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nimby_graph::models::{TrainJourney, SegmentState, detect_line_conflicts};
use nimby_graph::data::parse_csv_data;
use std::collections::HashSet;

fn benchmark_conflict_detection(c: &mut Criterion) {
    // Load real data from lines.csv
    let (lines, stations) = parse_csv_data();
    let journeys = TrainJourney::generate_journeys(&lines, &stations);
    let station_names: Vec<String> = stations.iter().map(|s| s.name.clone()).collect();

    let segment_state = SegmentState {
        double_tracked_segments: HashSet::new(),
    };

    // Benchmark journey generation
    c.bench_function("generate_journeys", |b| {
        b.iter(|| {
            TrainJourney::generate_journeys(black_box(&lines), black_box(&stations))
        });
    });

    // Benchmark conflict detection
    c.bench_function("conflict_detection", |b| {
        b.iter(|| {
            detect_line_conflicts(
                black_box(&journeys),
                black_box(&station_names),
                black_box(&segment_state),
            )
        });
    });

    // Benchmark the full pipeline (what happens on every change)
    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let journeys = TrainJourney::generate_journeys(black_box(&lines), black_box(&stations));
            detect_line_conflicts(
                black_box(&journeys),
                black_box(&station_names),
                black_box(&segment_state),
            )
        });
    });
}

criterion_group!(benches, benchmark_conflict_detection);
criterion_main!(benches);