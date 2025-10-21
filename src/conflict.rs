use crate::constants::BASE_DATE;
use crate::models::{RailwayGraph, TrackDirection, Junctions};
use crate::time::time_to_fraction;
use crate::train_journey::TrainJourney;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use petgraph::visit::EdgeRef;

// Conflict detection constants
const STATION_MARGIN: chrono::Duration = chrono::Duration::seconds(30);
const PLATFORM_BUFFER: chrono::Duration = chrono::Duration::seconds(30);
const MAX_CONFLICTS: usize = 9999;

// Performance tracking for WASM builds
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_arch = "wasm32")]
static PLATFORM_CHECK_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static SEGMENT_CHECK_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static SEGMENT_PAIR_CALLS: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static SEGMENT_PAIR_TOTAL_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static REVERSE_EDGE_CHECK_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static SINGLE_TRACK_CHECK_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static BLOCK_VIOLATION_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static BLOCK_VIOLATION_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static INTERSECTION_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static INTERSECTION_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static SEGMENT_MAP_LOOKUP_TIME: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static LOOP_ITERATIONS: AtomicU64 = AtomicU64::new(0);
#[cfg(target_arch = "wasm32")]
static TIME_OVERLAP_CHECKS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConflictType {
    HeadOn,            // Trains meeting on same track, opposite directions
    Overtaking,        // Train catching up on same track, same direction
    BlockViolation,    // Two trains in same single-track block simultaneously
    PlatformViolation, // Two trains using same platform at same time
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Conflict {
    pub time: NaiveDateTime,
    pub position: f64, // Position between stations (0.0 to 1.0)
    pub station1_idx: usize,
    pub station2_idx: usize,
    pub journey1_id: String,
    pub journey2_id: String,
    pub conflict_type: ConflictType,
    // For block violations: store the time ranges of the two segments
    pub segment1_times: Option<(NaiveDateTime, NaiveDateTime)>,
    pub segment2_times: Option<(NaiveDateTime, NaiveDateTime)>,
    // For platform violations: store the platform index
    pub platform_idx: Option<usize>,
}

impl Conflict {
    /// Format a human-readable message describing the conflict (without timestamp)
    /// For `PlatformViolation` conflicts, caller should use `format_platform_message` instead for better performance
    #[must_use]
    pub fn format_message(&self, station1_name: &str, station2_name: &str, _graph: &RailwayGraph) -> String {
        match self.conflict_type {
            ConflictType::PlatformViolation => {
                format!(
                    "{} conflicts with {} at {} Platform ?",
                    self.journey1_id, self.journey2_id, station1_name
                )
            }
            ConflictType::HeadOn => {
                format!(
                    "{} conflicts with {} between {} and {}",
                    self.journey1_id, self.journey2_id, station1_name, station2_name
                )
            }
            ConflictType::Overtaking => {
                format!(
                    "{} overtakes {} between {} and {}",
                    self.journey2_id, self.journey1_id, station1_name, station2_name
                )
            }
            ConflictType::BlockViolation => {
                format!(
                    "{} block violation with {} between {} and {}",
                    self.journey1_id, self.journey2_id, station1_name, station2_name
                )
            }
        }
    }

    /// Format platform violation message with platform name provided (avoids graph lookup)
    #[must_use]
    pub fn format_platform_message(&self, station1_name: &str, platform_name: &str) -> String {
        format!(
            "{} conflicts with {} at {} Platform {}",
            self.journey1_id, self.journey2_id, station1_name, platform_name
        )
    }

    /// Get a short name for the conflict type
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self.conflict_type {
            ConflictType::HeadOn => "Head-on Conflict",
            ConflictType::Overtaking => "Overtaking",
            ConflictType::BlockViolation => "Block Violation",
            ConflictType::PlatformViolation => "Platform Violation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StationCrossing {
    pub time: NaiveDateTime,
    pub station_idx: usize,
    pub journey1_id: String,
    pub journey2_id: String,
}

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
    graph: &'a RailwayGraph,
    station_margin: chrono::Duration,
}

struct PlatformOccupancy {
    station_idx: usize,
    platform_idx: usize,
    time_start: NaiveDateTime,
    time_end: NaiveDateTime,
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
    graph: &RailwayGraph,
) -> (Vec<Conflict>, Vec<StationCrossing>) {
    #[cfg(not(target_arch = "wasm32"))]
    let total_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let total_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&format!("🔍 detect_line_conflicts START: {} journeys, {} nodes",
        train_journeys.len(), graph.graph.node_count()).into());

    // Reset performance counters
    #[cfg(target_arch = "wasm32")]
    {
        PLATFORM_CHECK_TIME.store(0, Ordering::Relaxed);
        SEGMENT_CHECK_TIME.store(0, Ordering::Relaxed);
        SEGMENT_PAIR_CALLS.store(0, Ordering::Relaxed);
        SEGMENT_PAIR_TOTAL_TIME.store(0, Ordering::Relaxed);
        REVERSE_EDGE_CHECK_TIME.store(0, Ordering::Relaxed);
        SINGLE_TRACK_CHECK_TIME.store(0, Ordering::Relaxed);
        BLOCK_VIOLATION_TIME.store(0, Ordering::Relaxed);
        BLOCK_VIOLATION_COUNT.store(0, Ordering::Relaxed);
        INTERSECTION_TIME.store(0, Ordering::Relaxed);
        INTERSECTION_COUNT.store(0, Ordering::Relaxed);
        SEGMENT_MAP_LOOKUP_TIME.store(0, Ordering::Relaxed);
        LOOP_ITERATIONS.store(0, Ordering::Relaxed);
        TIME_OVERLAP_CHECKS.store(0, Ordering::Relaxed);
    }

    let mut results = ConflictResults {
        conflicts: Vec::new(),
        station_crossings: Vec::new(),
    };

    // Pre-compute NodeIndex to display index mapping for O(1) lookups
    // Build mapping via BFS without cloning nodes (optimization for WASM)
    #[cfg(not(target_arch = "wasm32"))]
    let setup_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let setup_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let mut station_indices = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut display_idx = 0;

    if let Some(start_node) = graph.graph.node_indices().next() {
        queue.push_back(start_node);
        seen.insert(start_node);

        while let Some(node_idx) = queue.pop_front() {
            station_indices.insert(node_idx, display_idx);
            display_idx += 1;

            for edge in graph.graph.edges(node_idx) {
                let target = edge.target();
                if seen.insert(target) {
                    queue.push_back(target);
                }
            }
        }

        // Handle disconnected nodes
        for node_idx in graph.graph.node_indices() {
            if seen.insert(node_idx) {
                station_indices.insert(node_idx, display_idx);
                display_idx += 1;
            }
        }
    }

    let ctx = ConflictContext {
        station_indices,
        graph,
        station_margin: STATION_MARGIN,
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let setup_time = setup_start.elapsed();
        eprintln!("Setup time: {setup_time:?}");
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = setup_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("  Setup (BFS mapping): {:.2}ms", elapsed).into());
    }

    // For small datasets, use simple approach with early filtering
    // Spatial partitioning only helps with larger datasets
    if train_journeys.len() < 200 {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"  Using simple O(n²) algorithm (< 200 journeys)".into());

        // Simple O(n²) with early time filtering
        for (i, journey1) in train_journeys.iter().enumerate() {
            if results.conflicts.len() >= MAX_CONFLICTS {
                break;
            }

            let j1_start = journey1.station_times.first().map(|(_, arr, _)| *arr);
            let j1_end = journey1.station_times.last().map(|(_, _, dep)| *dep);

            for journey2 in train_journeys.iter().skip(i + 1) {
                // Early time-based filtering - skip if no overlap in time
                let Some((start1, end1)) = j1_start.zip(j1_end) else {
                    continue;
                };

                let Some((_, start2, _)) = journey2.station_times.first() else {
                    continue;
                };
                let Some((_, _, end2)) = journey2.station_times.last() else {
                    continue;
                };

                if end1 < *start2 || *end2 < start1 {
                    continue; // No time overlap
                }

                check_journey_pair(journey1, journey2, &ctx, &mut results);
                if results.conflicts.len() >= MAX_CONFLICTS {
                    break;
                }
            }
        }
    } else {
        detect_conflicts_sweep_line(train_journeys, &ctx, &mut results);
    }

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

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = total_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("✅ detect_line_conflicts COMPLETE: {:.2}ms - Found {} conflicts, {} crossings",
            elapsed, results.conflicts.len(), results.station_crossings.len()).into());
    }

    (results.conflicts, results.station_crossings)
}

/// Sweep-line algorithm for detecting conflicts in large datasets
#[inline]
fn detect_conflicts_sweep_line(
    train_journeys: &[TrainJourney],
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    // Sweep-line algorithm: sort journeys by start time, only compare overlapping ones
    // This gives us O(n * m) where m is the average number of overlapping journeys (much smaller than n)

    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&format!("  Using sweep-line algorithm ({} journeys)", train_journeys.len()).into());

    #[cfg(not(target_arch = "wasm32"))]
    let sort_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let sort_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

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

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = sort_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("    Sort time: {:.2}ms", elapsed).into());
    }

    #[cfg(not(target_arch = "wasm32"))]
    let mut comparisons = 0;

    #[cfg(not(target_arch = "wasm32"))]
    let comparison_start = std::time::Instant::now();

    // Pre-build all segment lookup maps and platform occupancies once
    #[cfg(not(target_arch = "wasm32"))]
    let cache_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let cache_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    #[cfg(target_arch = "wasm32")]
    let seg_map_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let segment_maps: Vec<_> = train_journeys
        .iter()
        .map(|journey| build_segment_lookup_map(journey, ctx))
        .collect();

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = seg_map_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("      Segment maps: {:.2}ms", elapsed).into());
    }

    #[cfg(target_arch = "wasm32")]
    let plat_occ_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let platform_occupancies: Vec<_> = train_journeys
        .iter()
        .map(|journey| extract_platform_occupancies(journey, ctx))
        .collect();

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = plat_occ_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("      Platform occupancies: {:.2}ms", elapsed).into());
    }

    #[cfg(target_arch = "wasm32")]
    let seg_list_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    // Pre-build segment lists with resolved indices and pre-computed bounds for all journeys
    let segment_lists: Vec<_> = train_journeys
        .iter()
        .map(|journey| build_segment_list_with_bounds(journey, ctx))
        .collect();

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = seg_list_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("      Segment lists: {:.2}ms", elapsed).into());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let cache_time = cache_start.elapsed();
        eprintln!("Segment map & platform cache build time: {cache_time:?}");
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = cache_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        web_sys::console::log_1(&format!("    Cache build time: {:.2}ms", elapsed).into());
    }

    #[cfg(target_arch = "wasm32")]
    let loop_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    // For each journey, only compare with journeys that could overlap in time
    for i in 0..journey_times.len() {
        if results.conflicts.len() >= MAX_CONFLICTS {
            break;
        }

        let (start_i, end_i, idx_i) = journey_times[i];
        let journey_i = &train_journeys[idx_i];
        let seg_map_i = &segment_maps[idx_i];
        let plat_occ_i = &platform_occupancies[idx_i];
        let seg_list_i = &segment_lists[idx_i];

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

            #[cfg(not(target_arch = "wasm32"))]
            {
                comparisons += 1;
            }

            let journey_j = &train_journeys[*idx_j];
            let seg_map_j = &segment_maps[*idx_j];
            let plat_occ_j = &platform_occupancies[*idx_j];
            let seg_list_j = &segment_lists[*idx_j];
            check_journey_pair_with_all_cached(journey_i, journey_j, ctx, results, seg_map_i, seg_map_j, plat_occ_i, plat_occ_j, seg_list_i, seg_list_j);

            if results.conflicts.len() >= MAX_CONFLICTS {
                break;
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let comparison_time = comparison_start.elapsed();
        eprintln!("Comparison loop time: {comparison_time:?}");
        eprintln!("Made {comparisons} comparisons (vs {} for naive O(n²))", train_journeys.len() * (train_journeys.len() - 1) / 2);
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = loop_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        let platform_total_us = PLATFORM_CHECK_TIME.load(Ordering::Relaxed);
        let segment_total_us = SEGMENT_CHECK_TIME.load(Ordering::Relaxed);
        let segment_pair_calls = SEGMENT_PAIR_CALLS.load(Ordering::Relaxed);
        let segment_pair_total_us = SEGMENT_PAIR_TOTAL_TIME.load(Ordering::Relaxed);
        let reverse_edge_time_us = REVERSE_EDGE_CHECK_TIME.load(Ordering::Relaxed);
        let single_track_time_us = SINGLE_TRACK_CHECK_TIME.load(Ordering::Relaxed);
        let block_violation_time_us = BLOCK_VIOLATION_TIME.load(Ordering::Relaxed);
        let block_violation_count = BLOCK_VIOLATION_COUNT.load(Ordering::Relaxed);
        let intersection_time_us = INTERSECTION_TIME.load(Ordering::Relaxed);
        let intersection_count = INTERSECTION_COUNT.load(Ordering::Relaxed);
        let segment_map_lookup_us = SEGMENT_MAP_LOOKUP_TIME.load(Ordering::Relaxed);
        let loop_iterations = LOOP_ITERATIONS.load(Ordering::Relaxed);
        let time_overlap_checks = TIME_OVERLAP_CHECKS.load(Ordering::Relaxed);

        web_sys::console::log_1(&format!("    Comparison loop time: {:.2}ms", elapsed).into());
        web_sys::console::log_1(&format!("      Platform checks: {:.2}ms", platform_total_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("      Segment checks: {:.2}ms", segment_total_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("        Loop iterations: {}", loop_iterations).into());
        web_sys::console::log_1(&format!("        Time overlap checks: {}", time_overlap_checks).into());
        web_sys::console::log_1(&format!("        Segment pair calls: {}", segment_pair_calls).into());
        web_sys::console::log_1(&format!("        Segment pair total time: {:.2}ms", segment_pair_total_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("          HashMap lookups: {:.2}ms", segment_map_lookup_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("          Reverse edge checks: {:.2}ms", reverse_edge_time_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("          Single track checks: {:.2}ms", single_track_time_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("          Block violations: {} found, {:.2}ms total", block_violation_count, block_violation_time_us as f64 / 1000.0).into());
        web_sys::console::log_1(&format!("          Intersections: {} found, {:.2}ms total", intersection_count, intersection_time_us as f64 / 1000.0).into());
    }
}

fn check_journey_pair(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    // Pre-build segment lookup maps for both journeys to avoid O(n) lookups in inner loops
    let seg1_map = build_segment_lookup_map(journey1, ctx);
    let seg2_map = build_segment_lookup_map(journey2, ctx);

    check_journey_pair_with_maps(journey1, journey2, ctx, results, &seg1_map, &seg2_map);
}

fn check_journey_pair_with_maps(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
    seg1_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    seg2_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
) {
    // Check for platform conflicts first
    #[cfg(not(target_arch = "wasm32"))]
    let platform_start = std::time::Instant::now();

    check_platform_conflicts(journey1, journey2, ctx, results);

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::PLATFORM_TIME, platform_start.elapsed());

    check_segments_for_pair(journey1, journey2, ctx, results, seg1_map, seg2_map);
}

#[allow(clippy::too_many_arguments)]
fn check_journey_pair_with_all_cached(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
    seg1_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    seg2_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    plat_occ1: &[PlatformOccupancy],
    plat_occ2: &[PlatformOccupancy],
    seg_list1: &[CachedSegment],
    seg_list2: &[CachedSegment],
) {
    // Check for platform conflicts first using pre-cached occupancies
    #[cfg(not(target_arch = "wasm32"))]
    let platform_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let platform_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    check_platform_conflicts_cached(journey1, journey2, results, plat_occ1, plat_occ2);

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::PLATFORM_TIME, platform_start.elapsed());

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = platform_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        // Store as microseconds to preserve decimal precision
        PLATFORM_CHECK_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }

    #[cfg(target_arch = "wasm32")]
    let segment_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    check_segments_for_pair_cached(journey1, journey2, ctx, results, seg1_map, seg2_map, seg_list1, seg_list2);

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = segment_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        // Store as microseconds to preserve decimal precision
        SEGMENT_CHECK_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }
}

#[allow(clippy::similar_names)]
fn check_segments_for_pair(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
    seg1_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    seg2_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
) {
    let segments1 = build_segment_list_with_bounds(journey1, ctx);
    let segments2 = build_segment_list_with_bounds(journey2, ctx);

    check_segments_for_pair_cached(journey1, journey2, ctx, results, seg1_map, seg2_map, &segments1, &segments2);
}

#[allow(clippy::similar_names)]
fn check_segments_for_pair_cached(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
    seg1_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    seg2_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    segments1: &[CachedSegment],
    segments2: &[CachedSegment],
) {
    // Check all segment pairs using binary search to find overlapping ranges
    for cached1 in segments1 {
        let seg1 = &cached1.segment;

        // Binary search to find first segment in segments2 that could overlap with seg1
        // We're looking for the first segment where segment2.time_end >= seg1.time_start
        let start_idx = segments2.partition_point(|cached2| cached2.segment.time_end < seg1.time_start);

        // Iterate only through segments that could possibly overlap
        for cached2 in &segments2[start_idx..] {
            #[cfg(target_arch = "wasm32")]
            LOOP_ITERATIONS.fetch_add(1, Ordering::Relaxed);

            let seg2 = &cached2.segment;

            // If seg1 ends before seg2 starts, no more overlaps possible
            if seg1.time_end < seg2.time_start {
                break;
            }

            check_segment_pair(
                seg1, seg2, cached1.idx_min, cached1.idx_max, journey1, journey2, ctx, results, seg1_map, seg2_map,
            );

            if results.conflicts.len() >= MAX_CONFLICTS {
                return;
            }
        }
    }
}

/// Build a lookup map from (`start_idx`, `end_idx`) -> segment info for fast lookups
fn build_segment_lookup_map<'a>(
    journey: &'a TrainJourney,
    ctx: &ConflictContext,
) -> HashMap<(usize, usize), &'a crate::train_journey::JourneySegment> {
    let mut map = HashMap::new();

    for (i, _) in journey.station_times.iter().enumerate().skip(1) {
        if i - 1 < journey.segments.len() {
            let node_idx1 = &journey.station_times[i - 1].0;
            let node_idx2 = &journey.station_times[i].0;

            if let (Some(&s1_idx), Some(&s2_idx)) = (
                ctx.station_indices.get(node_idx1),
                ctx.station_indices.get(node_idx2),
            ) {
                map.insert((s1_idx, s2_idx), &journey.segments[i - 1]);
            }
        }
    }

    map
}

/// Segment with pre-computed spatial bounds for faster checking
#[derive(Debug, Clone, Copy)]
struct CachedSegment {
    segment: JourneySegment,
    idx_min: usize,
    idx_max: usize,
}

/// Build a list of journey segments with resolved station indices and pre-computed bounds
fn build_segment_list_with_bounds(journey: &TrainJourney, ctx: &ConflictContext) -> Vec<CachedSegment> {
    let mut segments = Vec::new();
    let mut prev: Option<(NaiveDateTime, usize)> = None;

    for (node_idx, arrival_time, departure_time) in &journey.station_times {
        let Some(&station_idx) = ctx.station_indices.get(node_idx) else {
            continue;
        };

        if let Some((prev_departure_time, prev_idx)) = prev {
            let segment = JourneySegment {
                time_start: prev_departure_time,
                time_end: *arrival_time,
                idx_start: prev_idx,
                idx_end: station_idx,
            };
            segments.push(CachedSegment {
                segment,
                idx_min: prev_idx.min(station_idx),
                idx_max: prev_idx.max(station_idx),
            });
        }
        prev = Some((*departure_time, station_idx));
    }

    segments
}

#[allow(clippy::too_many_arguments, clippy::similar_names, clippy::too_many_lines)]
fn check_segment_pair(
    segment1: &JourneySegment,
    segment2: &JourneySegment,
    seg1_min: usize,
    seg1_max: usize,
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
    segment1_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
    segment2_map: &HashMap<(usize, usize), &crate::train_journey::JourneySegment>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::Ordering;
        timing::SEGMENT_PAIR_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(target_arch = "wasm32")]
    let pair_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    #[cfg(target_arch = "wasm32")]
    SEGMENT_PAIR_CALLS.fetch_add(1, Ordering::Relaxed);

    // Check if the segments overlap in space
    let seg2_min = segment2.idx_start.min(segment2.idx_end);
    let seg2_max = segment2.idx_start.max(segment2.idx_end);

    if seg1_max <= seg2_min || seg2_max <= seg1_min {
        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return;
    }

    // Get segment info from pre-built lookup maps - O(1) instead of O(n)
    #[cfg(not(target_arch = "wasm32"))]
    let lookup_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let lookup_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let seg1_info = segment1_map.get(&(segment1.idx_start, segment1.idx_end));
    let seg2_info = segment2_map.get(&(segment2.idx_start, segment2.idx_end));

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::LOOKUP_TIME, lookup_start.elapsed());

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = lookup_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        SEGMENT_MAP_LOOKUP_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }

    // Both segments must have track info to check for conflicts
    let (Some(info1), Some(info2)) = (seg1_info, seg2_info) else {
        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return;
    };

    // Check if they're on the same edge OR reverse edges on the same bidirectional track
    let same_edge = info1.edge_index == info2.edge_index;

    #[cfg(target_arch = "wasm32")]
    let reverse_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let reverse_edges = are_reverse_bidirectional_edges(
        ctx,
        info1.edge_index,
        info2.edge_index,
        info1.track_index,
        info2.track_index,
        (segment1.idx_start, segment1.idx_end),
        (segment2.idx_start, segment2.idx_end),
    );

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = reverse_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        REVERSE_EDGE_CHECK_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }

    if !same_edge && !reverse_edges {
        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return; // Different edges, no conflict
    }

    // Check if they're on the same track (only if same edge, reverse edges already checked track)
    if same_edge && info1.track_index != info2.track_index {
        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return; // Different tracks on same edge, no conflict
    }

    // Determine travel directions
    let same_direction = (segment1.idx_start < segment1.idx_end
        && segment2.idx_start < segment2.idx_end)
        || (segment1.idx_start > segment1.idx_end && segment2.idx_start > segment2.idx_end);

    #[cfg(target_arch = "wasm32")]
    let single_track_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let is_single_track = is_single_track_bidirectional(ctx, info1.edge_index);

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = single_track_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        SINGLE_TRACK_CHECK_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }

    // For same-direction on single-track, check time overlap (block violation)
    if same_direction && is_single_track {
        // Check if time ranges overlap
        let time_overlap =
            segment1.time_start < segment2.time_end && segment2.time_start < segment1.time_end;

        if time_overlap {
            #[cfg(target_arch = "wasm32")]
            let block_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());
            // Two trains on same single-track block at same time, same direction = block violation
            // Conflict occurs when the trailing train enters while leading train is still in block
            let conflict_time = segment1.time_start.max(segment2.time_start);

            // Calculate where the leading train is when the trailing train enters
            let (leading_start, leading_end) = if segment1.time_start < segment2.time_start {
                (segment1.time_start, segment1.time_end)
            } else {
                (segment2.time_start, segment2.time_end)
            };

            // Calculate progress of leading train at conflict time
            // Break down durations to avoid precision loss in i64 to f64 conversion
            let duration = leading_end - leading_start;
            let elapsed = conflict_time - leading_start;

            let mut position = if duration.num_milliseconds() > 0 {
                // Use floating point division on Duration to avoid precision loss
                // Casts truncate for very large durations, but are correct for typical journey segments
                #[allow(clippy::cast_possible_truncation)]
                let elapsed_secs = f64::from(elapsed.num_seconds() as i32);
                #[allow(clippy::cast_possible_truncation)]
                let elapsed_subsec_ms = f64::from((elapsed.num_milliseconds() % 1000) as i32);
                #[allow(clippy::cast_possible_truncation)]
                let duration_secs = f64::from(duration.num_seconds() as i32);
                #[allow(clippy::cast_possible_truncation)]
                let duration_subsec_ms = f64::from((duration.num_milliseconds() % 1000) as i32);

                let elapsed_total = elapsed_secs + elapsed_subsec_ms / 1000.0;
                let duration_total = duration_secs + duration_subsec_ms / 1000.0;

                (elapsed_total / duration_total).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // If traveling backward (from higher to lower index), invert position
            // because rendering expects position relative to seg1_min -> seg1_max
            let traveling_backward = segment1.idx_start > segment1.idx_end;
            if traveling_backward {
                position = 1.0 - position;
            }

            results.conflicts.push(Conflict {
                time: conflict_time,
                position,
                station1_idx: seg1_min,
                station2_idx: seg1_max,
                journey1_id: journey1.train_number.clone(),
                journey2_id: journey2.train_number.clone(),
                conflict_type: ConflictType::BlockViolation,
                segment1_times: Some((segment1.time_start, segment1.time_end)),
                segment2_times: Some((segment2.time_start, segment2.time_end)),
                platform_idx: None,
            });

            #[cfg(target_arch = "wasm32")]
            if let Some(elapsed) = block_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
                BLOCK_VIOLATION_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
                BLOCK_VIOLATION_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return;
    }

    // For all other cases, calculate geometric intersection
    #[cfg(not(target_arch = "wasm32"))]
    let intersection_start = std::time::Instant::now();

    #[cfg(target_arch = "wasm32")]
    let intersection_start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

    let Some(intersection) = calculate_intersection(
        segment1.time_start,
        segment1.time_end,
        segment1.idx_start,
        segment1.idx_end,
        segment2.time_start,
        segment2.time_end,
        segment2.idx_start,
        segment2.idx_end,
    ) else {
        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return;
    };

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::INTERSECTION_TIME, intersection_start.elapsed());

    // Check if crossing happens very close to a station
    if is_near_station(&intersection, segment1, segment2, ctx.station_margin) {
        // This is a successful station crossing - add it to the list
        let station_idx = find_nearest_station(&intersection, segment1, segment2);
        results.station_crossings.push(StationCrossing {
            time: intersection.time,
            station_idx,
            journey1_id: journey1.train_number.clone(),
            journey2_id: journey2.train_number.clone(),
        });

        #[cfg(target_arch = "wasm32")]
        if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        }
        return;
    }

    // Determine conflict type based on track type
    let conflict_type = if is_single_track {
        ConflictType::BlockViolation
    } else if same_direction {
        ConflictType::Overtaking
    } else {
        ConflictType::HeadOn
    };

    results.conflicts.push(Conflict {
        time: intersection.time,
        position: intersection.position,
        station1_idx: seg1_min,
        station2_idx: seg1_max,
        journey1_id: journey1.train_number.clone(),
        journey2_id: journey2.train_number.clone(),
        conflict_type,
        segment1_times: None,
        segment2_times: None,
        platform_idx: None,
    });

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = intersection_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        INTERSECTION_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
        INTERSECTION_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(elapsed) = pair_start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
        SEGMENT_PAIR_TOTAL_TIME.fetch_add((elapsed * 1000.0) as u64, Ordering::Relaxed);
    }
}


/// Check if two edges are reverse edges connecting the same stations with bidirectional tracks
fn are_reverse_bidirectional_edges(
    ctx: &ConflictContext,
    edge1_index: usize,
    edge2_index: usize,
    track1_index: usize,
    track2_index: usize,
    seg1: (usize, usize),
    seg2: (usize, usize),
) -> bool {
    let (seg1_start, seg1_end) = seg1;
    let (seg2_start, seg2_end) = seg2;

    // Check if the segments connect the same stations in reverse order
    // seg1 goes from seg1_start to seg1_end
    // seg2 goes from seg2_start to seg2_end
    // They're reverse if seg1_start == seg2_end AND seg1_end == seg2_start
    let connects_reverse = seg1_start == seg2_end && seg1_end == seg2_start;

    if !connects_reverse {
        return false;
    }

    // Both must be using the same track index
    if track1_index != track2_index {
        return false;
    }

    // For reverse edges to conflict, they must be on tracks that allow bidirectional travel
    // This only applies to single-track bidirectional sections, not double-track sections
    let edge1_idx = petgraph::graph::EdgeIndex::new(edge1_index);
    let edge2_idx = petgraph::graph::EdgeIndex::new(edge2_index);

    // Check if both tracks are bidirectional (single-track case)
    let edge1_bidir = ctx
        .graph
        .graph
        .edge_weight(edge1_idx)
        .and_then(|ts| ts.tracks.get(track1_index))
        .is_some_and(|t| matches!(t.direction, TrackDirection::Bidirectional));

    let edge2_bidir = ctx
        .graph
        .graph
        .edge_weight(edge2_idx)
        .and_then(|ts| ts.tracks.get(track2_index))
        .is_some_and(|t| matches!(t.direction, TrackDirection::Bidirectional));

    edge1_bidir && edge2_bidir
}

/// Check if an edge has only 1 bidirectional track (single-track section)
fn is_single_track_bidirectional(ctx: &ConflictContext, edge_index: usize) -> bool {
    let edge_idx = petgraph::graph::EdgeIndex::new(edge_index);

    if let Some(track_segment) = ctx.graph.graph.edge_weight(edge_idx) {
        if track_segment.tracks.len() == 1 {
            return matches!(
                track_segment.tracks[0].direction,
                TrackDirection::Bidirectional
            );
        }
    }

    false
}

fn is_near_station(
    intersection: &Intersection,
    segment1: &JourneySegment,
    segment2: &JourneySegment,
    station_margin: chrono::Duration,
) -> bool {
    // Only check the 4 relevant station times instead of all station times
    let times = [
        segment1.time_start,
        segment1.time_end,
        segment2.time_start,
        segment2.time_end,
    ];

    times
        .iter()
        .any(|t| (*t - intersection.time).abs() < station_margin)
}

fn find_nearest_station(
    intersection: &Intersection,
    segment1: &JourneySegment,
    segment2: &JourneySegment,
) -> usize {
    // Check which station is closest to the intersection time
    let times_with_idx = [
        (segment1.time_start, segment1.idx_start),
        (segment1.time_end, segment1.idx_end),
        (segment2.time_start, segment2.idx_start),
        (segment2.time_end, segment2.idx_end),
    ];

    times_with_idx
        .iter()
        .min_by_key(|(t, _)| (*t - intersection.time).abs())
        .map_or(segment1.idx_start, |(_, idx)| *idx)
}

#[derive(Debug)]
struct Intersection {
    time: NaiveDateTime,
    position: f64, // Position between stations (0.0 to 1.0)
}

#[allow(clippy::too_many_arguments)]
fn calculate_intersection(
    t1_start: NaiveDateTime,
    t1_end: NaiveDateTime,
    s1_start: usize,
    s1_end: usize,
    t2_start: NaiveDateTime,
    t2_end: NaiveDateTime,
    s2_start: usize,
    s2_end: usize,
) -> Option<Intersection> {
    // Convert times to fractions
    let x1_start = time_to_fraction(t1_start);
    let x1_end = time_to_fraction(t1_end);

    // Convert station indices to f64 for geometric calculations
    // f64 can represent integers up to 2^53 exactly, sufficient for any realistic station count
    #[allow(clippy::cast_precision_loss)]
    let y1_start = s1_start as f64;
    #[allow(clippy::cast_precision_loss)]
    let y1_end = s1_end as f64;

    let x2_start = time_to_fraction(t2_start);
    let x2_end = time_to_fraction(t2_end);
    #[allow(clippy::cast_precision_loss)]
    let y2_start = s2_start as f64;
    #[allow(clippy::cast_precision_loss)]
    let y2_end = s2_end as f64;

    // Calculate line intersection using parametric equations
    let denom =
        (x1_start - x1_end) * (y2_start - y2_end) - (y1_start - y1_end) * (x2_start - x2_end);

    if denom.abs() < 0.0001 {
        return None; // Lines are parallel
    }

    let t = ((x1_start - x2_start) * (y2_start - y2_end)
        - (y1_start - y2_start) * (x2_start - x2_end))
        / denom;
    let u = -((x1_start - x1_end) * (y1_start - y2_start)
        - (y1_start - y1_end) * (x1_start - x2_start))
        / denom;

    // Check if intersection is within both segments
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        let x_intersect = x1_start + t * (x1_end - x1_start);
        let y_intersect = y1_start + t * (y1_end - y1_start);

        // Convert back to time
        let base_datetime = BASE_DATE.and_hms_opt(0, 0, 0).expect("Valid datetime");
        #[allow(clippy::cast_possible_truncation)]
        let intersection_time =
            base_datetime + chrono::Duration::seconds((x_intersect * 3600.0) as i64);

        // Calculate position between stations
        let position = (y_intersect - y_intersect.floor()) % 1.0;

        Some(Intersection {
            time: intersection_time,
            position,
        })
    } else {
        None
    }
}

/// Extract all platform occupancies from a journey
fn extract_platform_occupancies(
    journey: &TrainJourney,
    ctx: &ConflictContext,
) -> Vec<PlatformOccupancy> {
    let mut occupancies = Vec::new();
    let buffer = PLATFORM_BUFFER;

    for (i, (node_idx, arrival_time, departure_time)) in
        journey.station_times.iter().enumerate()
    {
        let Some(&station_idx) = ctx.station_indices.get(node_idx) else {
            continue;
        };

        // Skip junctions - they don't have platforms
        if ctx.graph.is_junction(*node_idx) {
            continue;
        }

        // Determine which platform this journey uses at this station
        // A train can only occupy ONE platform at a time during a stop
        // Priority: use arrival platform (where train stops), or departure platform if no arrival
        let platform_idx = if i > 0 && i - 1 < journey.segments.len() {
            // Not the first station: use the destination platform of the previous segment (arrival platform)
            journey.segments[i - 1].destination_platform
        } else if i < journey.segments.len() {
            // First station: use the origin platform of the current segment (departure platform)
            journey.segments[i].origin_platform
        } else {
            // Single station (no segments) - use platform 0
            0
        };

        occupancies.push(PlatformOccupancy {
            station_idx,
            platform_idx,
            time_start: *arrival_time - buffer,
            time_end: *departure_time + buffer,
        });
    }

    occupancies
}

/// Check for platform conflicts between two journeys
fn check_platform_conflicts(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let extract_start = std::time::Instant::now();

    let occupancies1 = extract_platform_occupancies(journey1, ctx);
    let occupancies2 = extract_platform_occupancies(journey2, ctx);

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::PLATFORM_EXTRACT_TIME, extract_start.elapsed());

    check_platform_conflicts_cached(journey1, journey2, results, &occupancies1, &occupancies2);
}

/// Check for platform conflicts using pre-cached occupancies
fn check_platform_conflicts_cached(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    results: &mut ConflictResults,
    occupancies1: &[PlatformOccupancy],
    occupancies2: &[PlatformOccupancy],
) {
    #[cfg(not(target_arch = "wasm32"))]
    let compare_start = std::time::Instant::now();

    for occ1 in occupancies1 {
        for occ2 in occupancies2 {
            // Check if same station and same platform
            if occ1.station_idx != occ2.station_idx || occ1.platform_idx != occ2.platform_idx {
                continue;
            }

            // Check if time ranges overlap
            if occ1.time_start < occ2.time_end && occ2.time_start < occ1.time_end {
                // Platform conflict detected
                let conflict_time = occ1.time_start.max(occ2.time_start);

                results.conflicts.push(Conflict {
                    time: conflict_time,
                    position: 0.0, // Platform conflicts occur at a station, not between stations
                    station1_idx: occ1.station_idx,
                    station2_idx: occ1.station_idx,
                    journey1_id: journey1.train_number.clone(),
                    journey2_id: journey2.train_number.clone(),
                    conflict_type: ConflictType::PlatformViolation,
                    segment1_times: Some((occ1.time_start, occ1.time_end)),
                    segment2_times: Some((occ2.time_start, occ2.time_end)),
                    platform_idx: Some(occ1.platform_idx),
                });

                if results.conflicts.len() >= MAX_CONFLICTS {
                    return;
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::PLATFORM_COMPARE_TIME, compare_start.elapsed());
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };

        let message = conflict.format_message("Station 1", "Station 2", &graph);
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
        };

        let message = conflict.format_message("Central Station", "Central Station", &graph);
        assert_eq!(message, "Train A conflicts with Train B at Central Station Platform 2");
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
        };

        let message = conflict.format_message("A", "B", &graph);
        assert_eq!(message, "Fast overtakes Slow between A and B");
    }

    #[test]
    fn test_detect_line_conflicts_empty() {
        let graph = RailwayGraph::new();
        let journeys = vec![];

        let (conflicts, crossings) = detect_line_conflicts(&journeys, &graph);

        assert_eq!(conflicts.len(), 0);
        assert_eq!(crossings.len(), 0);
    }

    #[test]
    fn test_detect_line_conflicts_single_journey() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("A".to_string());
        let idx2 = graph.add_or_get_station("B".to_string());
        let edge = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);

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
        };

        let (conflicts, _) = detect_line_conflicts(&[journey], &graph);
        assert_eq!(conflicts.len(), 0);
    }

    #[test]
    fn test_is_single_track_bidirectional() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("A".to_string());
        let idx2 = graph.add_or_get_station("B".to_string());

        // Single bidirectional track
        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Double track
        let edge2 = graph.add_track(idx1, idx2, vec![
            Track { direction: TrackDirection::Forward },
            Track { direction: TrackDirection::Backward },
        ]);

        let ctx = ConflictContext {
            station_indices: HashMap::new(),
            graph: &graph,
            station_margin: STATION_MARGIN,
        };

        assert!(is_single_track_bidirectional(&ctx, edge1.index()));
        assert!(!is_single_track_bidirectional(&ctx, edge2.index()));
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
        let intersection = calculate_intersection(
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
        let intersection = calculate_intersection(
            t1_start, t1_end, 0, 1,
            t2_start, t2_end, 1, 0,
        );

        assert!(intersection.is_none());
    }
}
