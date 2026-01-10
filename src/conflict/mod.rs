mod geometry;
mod platform;
mod segment;
mod types;

pub use types::{Conflict, ConflictType, SerializableConflictContext, StationCrossing};
use platform::PlatformOccupancy;
use segment::CachedSegment;

#[allow(unused_imports)]
use crate::logging::log;
use crate::train_journey::TrainJourney;
use chrono::NaiveDateTime;
use std::collections::HashMap;

// Conflict detection constants
#[cfg(test)]
const STATION_MARGIN: chrono::Duration = chrono::Duration::seconds(30);
#[cfg(test)]
const PLATFORM_BUFFER: chrono::Duration = chrono::Duration::seconds(30);
const MAX_CONFLICTS: usize = 9999;

/// Bitmap for fast station set intersection checks.
/// Dynamically sized based on max station index.
/// Intersection check is O(words) bitwise ops vs `HashSet`'s O(min(n,m)) iteration.
#[derive(Clone)]
struct StationBitmap {
    words: Vec<u64>,
}

impl StationBitmap {
    fn new(max_station_idx: usize) -> Self {
        let num_words = (max_station_idx / 64) + 1;
        Self {
            words: vec![0; num_words],
        }
    }

    #[inline]
    fn insert(&mut self, station_idx: usize) {
        let word = station_idx / 64;
        let bit = station_idx % 64;
        if word < self.words.len() {
            self.words[word] |= 1 << bit;
        }
    }

    /// Returns true if the two bitmaps share at least one station
    #[inline]
    fn intersects(&self, other: &Self) -> bool {
        let len = self.words.len().min(other.words.len());
        for i in 0..len {
            if self.words[i] & other.words[i] != 0 {
                return true;
            }
        }
        false
    }
}

// Performance tracking for WASM builds (enabled with perf_timing feature)
#[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
use wasm_bindgen::JsCast;

/// Get Performance API from either window (main thread) or worker global scope
#[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
fn get_performance() -> Option<web_sys::Performance> {
    // Try window first (main thread)
    if let Some(window) = web_sys::window() {
        return window.performance();
    }
    // Fall back to worker global scope
    let global = js_sys::global();
    let worker_scope: web_sys::WorkerGlobalScope = global.dyn_into().ok()?;
    worker_scope.performance()
}
// Phase-level timing accumulators (no per-iteration counters for performance)
#[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
static PLATFORM_CHECK_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
static SEGMENT_CHECK_TIME: AtomicU64 = AtomicU64::new(0);

struct ConflictResults {
    conflicts: Vec<Conflict>,
    station_crossings: Vec<StationCrossing>,
}

#[derive(Debug, Clone, Copy)]
struct JourneySegment {
    time_start: NaiveDateTime,
    time_end: NaiveDateTime,
    idx_start: usize,
    idx_end: usize,
}

struct ConflictContext<'a> {
    station_indices: HashMap<petgraph::stable_graph::NodeIndex, usize>,
    serializable_ctx: &'a SerializableConflictContext,
    station_margin: chrono::Duration,
    minimum_separation: chrono::Duration,
    ignore_same_direction_platform_conflicts: bool,
}

#[cfg(not(target_arch = "wasm32"))]
mod timing {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    pub static PLATFORM_TIME: AtomicU64 = AtomicU64::new(0);
    pub static PLATFORM_EXTRACT_TIME: AtomicU64 = AtomicU64::new(0);
    pub static PLATFORM_COMPARE_TIME: AtomicU64 = AtomicU64::new(0);
    pub static SEGMENT_TIME: AtomicU64 = AtomicU64::new(0);
    pub static SEGMENT_PAIR_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static LOOKUP_TIME: AtomicU64 = AtomicU64::new(0);
    pub static INTERSECTION_TIME: AtomicU64 = AtomicU64::new(0);

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn add_duration(counter: &AtomicU64, duration: Duration) {
        counter.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }
}

#[must_use]
pub fn detect_line_conflicts(
    train_journeys: &[TrainJourney],
    serializable_ctx: &SerializableConflictContext,
) -> (Vec<Conflict>, Vec<StationCrossing>) {
    #[cfg(not(target_arch = "wasm32"))]
    let total_start = std::time::Instant::now();

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let total_start = get_performance().map(|p| p.now());

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    log!("🔍 detect_line_conflicts START: {} journeys, {} stations",
        train_journeys.len(), serializable_ctx.station_indices.len());

    // Reset performance counters
    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    {
        PLATFORM_CHECK_TIME.store(0, Ordering::Relaxed);
        SEGMENT_CHECK_TIME.store(0, Ordering::Relaxed);
    }

    let mut results = ConflictResults {
        conflicts: Vec::new(),
        station_crossings: Vec::new(),
    };

    // Convert serializable station_indices back to NodeIndex keys for internal use
    #[cfg(not(target_arch = "wasm32"))]
    let setup_start = std::time::Instant::now();

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let setup_start = get_performance().map(|p| p.now());

    let station_indices: HashMap<petgraph::stable_graph::NodeIndex, usize> = serializable_ctx.station_indices
        .iter()
        .map(|(&k, &v)| (petgraph::stable_graph::NodeIndex::new(k), v))
        .collect();

    let ctx = ConflictContext {
        station_indices,
        serializable_ctx,
        station_margin: chrono::Duration::seconds(serializable_ctx.station_margin_secs),
        minimum_separation: chrono::Duration::seconds(serializable_ctx.minimum_separation_secs),
        ignore_same_direction_platform_conflicts: serializable_ctx.ignore_same_direction_platform_conflicts,
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let setup_time = setup_start.elapsed();
        eprintln!("Setup time: {setup_time:?}");
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = setup_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        log!("  Setup (context conversion): {:.2}ms", elapsed);
    }

    detect_conflicts_sweep_line(train_journeys, &ctx, &mut results);

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::Ordering;

        let total_time = total_start.elapsed();
        eprintln!("Total detection time: {total_time:?}");
        eprintln!("Found {} conflicts, {} crossings from {} journeys", results.conflicts.len(), results.station_crossings.len(), train_journeys.len());

        // Print detailed timing breakdown
        eprintln!("\n=== Detailed Timing Breakdown ===");

        let platform_ns = timing::PLATFORM_TIME.load(Ordering::Relaxed);
        let segment_ns = timing::SEGMENT_TIME.load(Ordering::Relaxed);
        let segment_pair_calls = timing::SEGMENT_PAIR_CALLS.load(Ordering::Relaxed);
        let lookup_ns = timing::LOOKUP_TIME.load(Ordering::Relaxed);
        let intersection_ns = timing::INTERSECTION_TIME.load(Ordering::Relaxed);

        let platform_extract_ns = timing::PLATFORM_EXTRACT_TIME.load(Ordering::Relaxed);
        let platform_compare_ns = timing::PLATFORM_COMPARE_TIME.load(Ordering::Relaxed);

        #[allow(clippy::cast_precision_loss)]
        {
            eprintln!("Platform checks:     {:>10.3}ms", platform_ns as f64 / 1_000_000.0);
            eprintln!("  Extract occupancy: {:>10.3}ms", platform_extract_ns as f64 / 1_000_000.0);
            eprintln!("  Compare occupancy: {:>10.3}ms", platform_compare_ns as f64 / 1_000_000.0);
            eprintln!("Segment checks:      {:>10.3}ms", segment_ns as f64 / 1_000_000.0);
            eprintln!("  Segment pairs:     {segment_pair_calls:>10} calls");
            eprintln!("  HashMap lookups:   {:>10.3}ms", lookup_ns as f64 / 1_000_000.0);
            eprintln!("  Intersections:     {:>10.3}ms", intersection_ns as f64 / 1_000_000.0);
        }
        eprintln!("=================================");
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = total_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        log!("✅ detect_line_conflicts COMPLETE: {:.2}ms - Found {} conflicts, {} crossings",
            elapsed, results.conflicts.len(), results.station_crossings.len());
    }

    (results.conflicts, results.station_crossings)
}

/// Sweep-line algorithm for detecting conflicts in large datasets
#[inline]
#[allow(clippy::too_many_lines)]
fn detect_conflicts_sweep_line(
    train_journeys: &[TrainJourney],
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    // Sweep-line algorithm: sort journeys by start time, only compare overlapping ones
    // This gives us O(n * m) where m is the average number of overlapping journeys (much smaller than n)

    #[cfg(target_arch = "wasm32")]
    log!("  Using sweep-line algorithm ({} journeys)", train_journeys.len());

    #[cfg(not(target_arch = "wasm32"))]
    let sort_start = std::time::Instant::now();

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let sort_start = get_performance().map(|p| p.now());

    // Create sorted index array with (start_time, end_time, index)
    let mut journey_times: Vec<(NaiveDateTime, NaiveDateTime, usize)> = train_journeys
        .iter()
        .enumerate()
        .filter_map(|(idx, journey)| {
            if let (Some((_, start, _)), Some((_, _, end))) =
                (journey.station_times.first(), journey.station_times.last()) {
                Some((*start, *end, idx))
            } else {
                None
            }
        })
        .collect();

    // Sort by start time
    journey_times.sort_by_key(|(start, _, _)| *start);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let sort_time = sort_start.elapsed();
        eprintln!("Sort time: {sort_time:?}");
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = sort_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        log!("    Sort time: {:.2}ms", elapsed);
    }

    #[cfg(not(target_arch = "wasm32"))]
    let mut comparisons = 0;

    #[cfg(not(target_arch = "wasm32"))]
    let comparison_start = std::time::Instant::now();

    // Pre-build all segment lookup maps and platform occupancies once
    #[cfg(not(target_arch = "wasm32"))]
    let cache_start = std::time::Instant::now();

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let cache_start = get_performance().map(|p| p.now());

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let plat_occ_start = get_performance().map(|p| p.now());

    // Get max station index for bitmap sizing
    let max_station_idx = ctx.station_indices.len();

    // Build platform occupancies and station bitmaps for each journey
    let platform_data: Vec<_> = train_journeys
        .iter()
        .map(|journey| {
            let occupancies = platform::extract_platform_occupancies(journey, ctx);
            // Build bitmap of stations for fast intersection check
            let mut stations = StationBitmap::new(max_station_idx);
            for occ in &occupancies {
                stations.insert(occ.station_idx);
            }
            (occupancies, stations)
        })
        .collect();
    let platform_occupancies: Vec<_> = platform_data.iter().map(|(occs, _)| occs).collect();
    let station_bitmaps: Vec<_> = platform_data.iter().map(|(_, bm)| bm).collect();

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = plat_occ_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        log!("      Platform occupancies: {:.2}ms", elapsed);
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let seg_list_start = get_performance().map(|p| p.now());

    // Pre-build segment lists with resolved indices and pre-computed bounds for all journeys
    // Also build station pair sets for quick edge-sharing checks
    let segment_data: Vec<_> = train_journeys
        .iter()
        .map(|journey| {
            let segments = segment::build_segment_list_with_bounds(journey, ctx);
            // Build set of station pairs (as sorted tuples) for fast intersection check
            let station_pairs: std::collections::HashSet<(usize, usize)> = segments
                .iter()
                .map(|seg| {
                    let a = seg.segment.idx_start;
                    let b = seg.segment.idx_end;
                    (a.min(b), a.max(b))
                })
                .collect();
            (segments, station_pairs)
        })
        .collect();
    let segment_lists: Vec<_> = segment_data.iter().map(|(segs, _)| segs).collect();
    let station_pair_sets: Vec<_> = segment_data.iter().map(|(_, pairs)| pairs).collect();

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = seg_list_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        log!("      Segment lists: {:.2}ms", elapsed);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let cache_time = cache_start.elapsed();
        eprintln!("Segment map & platform cache build time: {cache_time:?}");
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = cache_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        log!("    Cache build time: {:.2}ms", elapsed);
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let loop_start = get_performance().map(|p| p.now());

    // For each journey, only compare with journeys that could overlap in time
    for i in 0..journey_times.len() {
        if results.conflicts.len() >= MAX_CONFLICTS {
            break;
        }

        let (start_i, end_i, idx_i) = journey_times[i];
        let journey_i = &train_journeys[idx_i];
        let plat_occ_i = &platform_occupancies[idx_i];
        let seg_list_i = segment_lists[idx_i];
        let station_pairs_i = station_pair_sets[idx_i];
        let stations_i = &station_bitmaps[idx_i];

        // Only check journeys that start before journey_i ends
        // Once we find a journey that starts after journey_i ends, we can stop
        for (start_j, end_j, idx_j) in journey_times.iter().skip(i + 1) {

            // If journey j starts after journey i ends, no more overlaps possible
            if *start_j >= end_i {
                break;
            }

            // Additional check: if journey i starts after journey j ends, skip
            if start_i >= *end_j {
                continue;
            }

            let stations_j = &station_bitmaps[*idx_j];
            let station_pairs_j = station_pair_sets[*idx_j];

            // Early skip: if no shared stations AND no shared station pairs, no conflicts possible
            let shares_stations = stations_i.intersects(stations_j);
            let shares_station_pairs = !station_pairs_i.is_disjoint(station_pairs_j);

            if !shares_stations && !shares_station_pairs {
                continue;
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                comparisons += 1;
            }

            let journey_j = &train_journeys[*idx_j];
            let plat_occ_j = &platform_occupancies[*idx_j];
            let seg_list_j = segment_lists[*idx_j];
            check_journey_pair_with_all_cached(
                journey_i, journey_j, ctx, results,
                plat_occ_i, plat_occ_j, seg_list_i, seg_list_j,
                shares_stations, shares_station_pairs,
            );

            if results.conflicts.len() >= MAX_CONFLICTS {
                break;
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let comparison_time = comparison_start.elapsed();
        eprintln!("Comparison loop time: {comparison_time:?}");
        let n = train_journeys.len();
        let naive_comparisons = n.saturating_mul(n.saturating_sub(1)) / 2;
        eprintln!("Made {comparisons} comparisons (vs {naive_comparisons} for naive O(n²))");
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = loop_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        let platform_total_ms = PLATFORM_CHECK_TIME.load(Ordering::Relaxed) as f64 / 1000.0;
        let segment_total_ms = SEGMENT_CHECK_TIME.load(Ordering::Relaxed) as f64 / 1000.0;
        let loop_overhead_ms = elapsed - platform_total_ms - segment_total_ms;

        log!("    Comparison loop time: {:.2}ms", elapsed);
        log!("      Platform checks: {:.2}ms", platform_total_ms);
        log!("      Segment checks: {:.2}ms", segment_total_ms);
        log!("      Loop overhead (is_disjoint + iteration): {:.2}ms", loop_overhead_ms);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_journey_pair_with_all_cached(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
    plat_occ1: &[PlatformOccupancy],
    plat_occ2: &[PlatformOccupancy],
    seg_list1: &[CachedSegment],
    seg_list2: &[CachedSegment],
    shares_stations: bool,
    shares_station_pairs: bool,
) {
    // Check for platform conflicts only if journeys share stations
    if shares_stations {
        #[cfg(not(target_arch = "wasm32"))]
        let platform_start = std::time::Instant::now();

        #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
        let platform_start = get_performance().map(|p| p.now());

        platform::check_platform_conflicts_cached(journey1, journey2, results, plat_occ1, plat_occ2, ctx);

        #[cfg(not(target_arch = "wasm32"))]
        timing::add_duration(&timing::PLATFORM_TIME, platform_start.elapsed());

        #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
        if let Some(elapsed) = platform_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
            PLATFORM_CHECK_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
    }

    // Skip segment comparison if journeys share no station pairs
    if !shares_station_pairs {
        return;
    }

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    let segment_start = get_performance().map(|p| p.now());

    segment::check_segments_for_pair_cached(journey1, journey2, ctx, results, seg_list1, seg_list2);

    #[cfg(all(target_arch = "wasm32", feature = "perf_timing"))]
    if let Some(elapsed) = segment_start.and_then(|s| get_performance().map(|p| p.now() - s)) {
        SEGMENT_CHECK_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::BASE_DATE;
    use crate::models::{RailwayGraph, Stations, Tracks, Track, TrackDirection};
    use crate::train_journey::JourneySegment;

    const TEST_COLOR: &str = "#FF0000";
    const TEST_THICKNESS: f64 = 2.0;

    #[test]
    fn test_conflict_type_name() {
        let conflict = Conflict {
            time: BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time"),
            position: 0.5,
            station1_idx: 0,
            station2_idx: 1,
            journey1_id: "J1".to_string(),
            journey2_id: "J2".to_string(),
            conflict_type: ConflictType::HeadOn,
            segment1_times: None,
            segment2_times: None,
            platform_idx: None,
            edge_index: Some(0),
            timing_uncertain: false,
            actual1_times: None,
            actual2_times: None,
        };

        assert_eq!(conflict.type_name(), "Head-on Conflict");
    }

    #[test]
    fn test_conflict_format_message_head_on() {
        let mut graph = RailwayGraph::new();
        graph.add_or_get_station("Station 1".to_string());
        graph.add_or_get_station("Station 2".to_string());

        let conflict = Conflict {
            time: BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time"),
            position: 0.5,
            station1_idx: 0,
            station2_idx: 1,
            journey1_id: "Train A".to_string(),
            journey2_id: "Train B".to_string(),
            conflict_type: ConflictType::HeadOn,
            segment1_times: None,
            segment2_times: None,
            platform_idx: None,
            edge_index: Some(0),
            timing_uncertain: false,
            actual1_times: None,
            actual2_times: None,
        };

        let message = conflict.format_message("Station 1", "Station 2");
        assert_eq!(message, "Train A conflicts with Train B between Station 1 and Station 2");
    }

    #[test]
    fn test_conflict_format_message_platform() {
        let mut graph = RailwayGraph::new();
        let station_idx = graph.add_or_get_station("Central Station".to_string());

        // Add platforms to the station
        if let Some(station_node) = graph.graph.node_weight_mut(station_idx) {
            if let Some(station) = station_node.as_station_mut() {
                station.platforms = vec![
                    crate::models::Platform { name: "1".to_string() },
                    crate::models::Platform { name: "2".to_string() },
                ];
            }
        }

        let conflict = Conflict {
            time: BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time"),
            position: 0.0,
            station1_idx: 0,
            station2_idx: 0,
            journey1_id: "Train A".to_string(),
            journey2_id: "Train B".to_string(),
            conflict_type: ConflictType::PlatformViolation,
            segment1_times: None,
            segment2_times: None,
            platform_idx: Some(1),
            edge_index: None,
            timing_uncertain: false,
            actual1_times: None,
            actual2_times: None,
        };

        let message = conflict.format_message("Central Station", "Central Station");
        assert_eq!(message, "Train A conflicts with Train B at Central Station Platform ?");
    }

    #[test]
    fn test_conflict_format_message_overtaking() {
        let mut graph = RailwayGraph::new();
        graph.add_or_get_station("A".to_string());
        graph.add_or_get_station("B".to_string());

        let conflict = Conflict {
            time: BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time"),
            position: 0.5,
            station1_idx: 0,
            station2_idx: 1,
            journey1_id: "Slow".to_string(),
            journey2_id: "Fast".to_string(),
            conflict_type: ConflictType::Overtaking,
            segment1_times: None,
            segment2_times: None,
            platform_idx: None,
            edge_index: Some(0),
            timing_uncertain: false,
            actual1_times: None,
            actual2_times: None,
        };

        let message = conflict.format_message("A", "B");
        assert_eq!(message, "Fast overtakes Slow between A and B");
    }

    #[test]
    fn test_detect_line_conflicts_empty() {
        let graph = RailwayGraph::new();
        let journeys = vec![];

        let station_indices = HashMap::new();
        let ctx = SerializableConflictContext::from_graph(&graph, station_indices, STATION_MARGIN, PLATFORM_BUFFER, false);
        let (conflicts, crossings) = detect_line_conflicts(&journeys, &ctx);

        assert_eq!(conflicts.len(), 0);
        assert_eq!(crossings.len(), 0);
    }

    #[test]
    fn test_detect_line_conflicts_single_journey() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("A".to_string());
        let idx2 = graph.add_or_get_station("B".to_string());
        let edge = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }], None);

        let line_id = uuid::Uuid::new_v4();
        let journey = TrainJourney {
            id: uuid::Uuid::new_v4(),
            line_id,
            train_number: "Line 1 0001".to_string(),
            departure_time: BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"),
            station_times: vec![
                (idx1, BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"), BASE_DATE.and_hms_opt(8, 1, 0).expect("valid time")),
                (idx2, BASE_DATE.and_hms_opt(8, 10, 0).expect("valid time"), BASE_DATE.and_hms_opt(8, 11, 0).expect("valid time")),
            ],
            segments: vec![JourneySegment {
                edge_index: edge.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
            }],
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            route_start_node: Some(idx1),
            route_end_node: Some(idx2),
            timing_inherited: vec![false, false], // Test journey with explicit timing
            is_forward: true,
        };

        let station_indices = graph.graph.node_indices()
            .enumerate()
            .map(|(idx, node_idx)| (node_idx, idx))
            .collect();
        let ctx = SerializableConflictContext::from_graph(&graph, station_indices, STATION_MARGIN, PLATFORM_BUFFER, false);
        let (conflicts, _) = detect_line_conflicts(&[journey], &ctx);
        assert_eq!(conflicts.len(), 0);
    }

    #[test]
    fn test_is_single_track_bidirectional() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("A".to_string());
        let idx2 = graph.add_or_get_station("B".to_string());

        // Single bidirectional track
        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }], None);

        // Double track
        let edge2 = graph.add_track(idx1, idx2, vec![
            Track { direction: TrackDirection::Forward },
            Track { direction: TrackDirection::Backward },
        ], None);

        let serializable_ctx = SerializableConflictContext::from_graph(&graph, HashMap::new(), STATION_MARGIN, PLATFORM_BUFFER, false);
        let ctx = ConflictContext {
            station_indices: HashMap::new(),
            serializable_ctx: &serializable_ctx,
            station_margin: STATION_MARGIN,
            minimum_separation: PLATFORM_BUFFER,
            ignore_same_direction_platform_conflicts: false,
        };

        assert!(segment::is_single_track_bidirectional(&ctx, edge1.index()));
        assert!(!segment::is_single_track_bidirectional(&ctx, edge2.index()));
    }

    #[test]
    fn test_station_crossing_equality() {
        let crossing1 = StationCrossing {
            time: BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time"),
            station_idx: 1,
            journey1_id: "J1".to_string(),
            journey2_id: "J2".to_string(),
        };

        let crossing2 = StationCrossing {
            time: BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time"),
            station_idx: 1,
            journey1_id: "J1".to_string(),
            journey2_id: "J2".to_string(),
        };

        assert_eq!(crossing1, crossing2);
    }

    #[test]
    fn test_conflict_type_equality() {
        assert_eq!(ConflictType::HeadOn, ConflictType::HeadOn);
        assert_eq!(ConflictType::Overtaking, ConflictType::Overtaking);
        assert_eq!(ConflictType::BlockViolation, ConflictType::BlockViolation);
        assert_eq!(ConflictType::PlatformViolation, ConflictType::PlatformViolation);
        assert_ne!(ConflictType::HeadOn, ConflictType::Overtaking);
    }

    #[test]
    fn test_calculate_intersection_parallel_lines() {
        let t1_start = BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time");
        let t1_end = BASE_DATE.and_hms_opt(8, 10, 0).expect("valid time");
        let t2_start = BASE_DATE.and_hms_opt(8, 5, 0).expect("valid time");
        let t2_end = BASE_DATE.and_hms_opt(8, 15, 0).expect("valid time");

        // Both going from station 0 to 1 (parallel)
        let intersection = geometry::calculate_intersection(
            t1_start, t1_end, 0, 1,
            t2_start, t2_end, 0, 1,
        );

        assert!(intersection.is_none());
    }

    #[test]
    fn test_calculate_intersection_no_overlap() {
        let t1_start = BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time");
        let t1_end = BASE_DATE.and_hms_opt(8, 10, 0).expect("valid time");
        let t2_start = BASE_DATE.and_hms_opt(8, 20, 0).expect("valid time");
        let t2_end = BASE_DATE.and_hms_opt(8, 30, 0).expect("valid time");

        // Different times, should not intersect
        let intersection = geometry::calculate_intersection(
            t1_start, t1_end, 0, 1,
            t2_start, t2_end, 1, 0,
        );

        assert!(intersection.is_none());
    }
}
