use crate::constants::BASE_DATE;
use crate::models::{RailwayGraph, TrackDirection};
use crate::time::time_to_fraction;
use crate::train_journey::TrainJourney;
use chrono::NaiveDateTime;
use std::collections::HashMap;

// Conflict detection constants
const STATION_MARGIN_MINUTES: i64 = 1;
const PLATFORM_BUFFER_MINUTES: i64 = 1;
const MAX_CONFLICTS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictType {
    HeadOn,            // Trains meeting on same track, opposite directions
    Overtaking,        // Train catching up on same track, same direction
    BlockViolation,    // Two trains in same single-track block simultaneously
    PlatformViolation, // Two trains using same platform at same time
}

#[derive(Debug, Clone, PartialEq)]
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
    #[must_use]
    pub fn format_message(&self, station1_name: &str, station2_name: &str) -> String {
        match self.conflict_type {
            ConflictType::PlatformViolation => {
                let platform_num = self.platform_idx.unwrap_or(0) + 1;
                format!(
                    "{} conflicts with {} at {} Platform {}",
                    self.journey1_id, self.journey2_id, station1_name, platform_num
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

#[derive(Debug, Clone, PartialEq)]
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

struct JourneySegment {
    time_start: NaiveDateTime,
    time_end: NaiveDateTime,
    idx_start: usize,
    idx_end: usize,
}

struct ConflictContext<'a> {
    station_indices: HashMap<&'a str, usize>,
    graph: &'a RailwayGraph,
    station_margin: chrono::Duration,
}

struct PlatformOccupancy {
    station_idx: usize,
    platform_idx: usize,
    time_start: NaiveDateTime,
    time_end: NaiveDateTime,
}

#[must_use]
pub fn detect_line_conflicts(
    train_journeys: &[TrainJourney],
    graph: &RailwayGraph,
) -> (Vec<Conflict>, Vec<StationCrossing>) {
    let mut results = ConflictResults {
        conflicts: Vec::new(),
        station_crossings: Vec::new(),
    };

    // Get ordered list of stations from the graph
    let stations = graph.get_all_stations_ordered();

    // Pre-compute station name to index mapping for O(1) lookups
    let station_indices: HashMap<&str, usize> = stations
        .iter()
        .enumerate()
        .map(|(idx, station)| (station.name.as_str(), idx))
        .collect();

    let ctx = ConflictContext {
        station_indices,
        graph,
        station_margin: chrono::Duration::minutes(STATION_MARGIN_MINUTES),
    };

    // Compare each pair of journeys
    for (i, journey1) in train_journeys.iter().enumerate() {
        if results.conflicts.len() >= MAX_CONFLICTS {
            break;
        }
        for journey2 in train_journeys.iter().skip(i + 1) {
            check_journey_pair(journey1, journey2, &ctx, &mut results);
            if results.conflicts.len() >= MAX_CONFLICTS {
                break;
            }
        }
    }

    (results.conflicts, results.station_crossings)
}

fn check_journey_pair(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    // Check for platform conflicts first
    check_platform_conflicts(journey1, journey2, ctx, results);

    let mut prev1: Option<(NaiveDateTime, usize)> = None;

    for (station1, arrival_time1, departure_time1) in &journey1.station_times {
        let Some(&station1_idx) = ctx.station_indices.get(station1.as_str()) else {
            continue;
        };

        if let Some((prev_departure_time1, prev_idx1)) = prev1 {
            let segment1 = JourneySegment {
                time_start: prev_departure_time1,
                time_end: *arrival_time1,
                idx_start: prev_idx1,
                idx_end: station1_idx,
            };
            check_segment_against_journey(&segment1, journey1, journey2, ctx, results);
        }
        prev1 = Some((*departure_time1, station1_idx));
    }
}

fn check_segment_against_journey(
    segment1: &JourneySegment,
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    let seg1_min = segment1.idx_start.min(segment1.idx_end);
    let seg1_max = segment1.idx_start.max(segment1.idx_end);

    let mut prev2: Option<(NaiveDateTime, usize)> = None;

    for (station2, arrival_time2, departure_time2) in &journey2.station_times {
        let Some(&station2_idx) = ctx.station_indices.get(station2.as_str()) else {
            continue;
        };

        if let Some((prev_departure_time2, prev_idx2)) = prev2 {
            let segment2 = JourneySegment {
                time_start: prev_departure_time2,
                time_end: *arrival_time2,
                idx_start: prev_idx2,
                idx_end: station2_idx,
            };

            check_segment_pair(
                segment1, &segment2, seg1_min, seg1_max, journey1, journey2, ctx, results,
            );
            if results.conflicts.len() >= MAX_CONFLICTS {
                return;
            }
        }
        prev2 = Some((*departure_time2, station2_idx));
    }
}

fn check_segment_pair(
    segment1: &JourneySegment,
    segment2: &JourneySegment,
    seg1_min: usize,
    seg1_max: usize,
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    // Check if the segments overlap in space
    let seg2_min = segment2.idx_start.min(segment2.idx_end);
    let seg2_max = segment2.idx_start.max(segment2.idx_end);

    if seg1_max <= seg2_min || seg2_max <= seg1_min {
        return;
    }

    // Get segment info from journeys for track-level checking
    // Find which segment indices these correspond to
    let seg1_info = find_journey_segment_info(journey1, segment1.idx_start, segment1.idx_end, ctx);
    let seg2_info = find_journey_segment_info(journey2, segment2.idx_start, segment2.idx_end, ctx);

    // Both segments must have track info to check for conflicts
    let (Some(info1), Some(info2)) = (seg1_info, seg2_info) else {
        return;
    };

    // Check if they're on the same edge OR reverse edges on the same bidirectional track
    let same_edge = info1.edge_index == info2.edge_index;
    let reverse_edges = are_reverse_bidirectional_edges(
        ctx,
        info1.edge_index,
        info2.edge_index,
        info1.track_index,
        info2.track_index,
        (segment1.idx_start, segment1.idx_end),
        (segment2.idx_start, segment2.idx_end),
    );

    if !same_edge && !reverse_edges {
        return; // Different edges, no conflict
    }

    // Check if they're on the same track (only if same edge, reverse edges already checked track)
    if same_edge && info1.track_index != info2.track_index {
        return; // Different tracks on same edge, no conflict
    }

    // Determine travel directions
    let same_direction = (segment1.idx_start < segment1.idx_end
        && segment2.idx_start < segment2.idx_end)
        || (segment1.idx_start > segment1.idx_end && segment2.idx_start > segment2.idx_end);

    let is_single_track = is_single_track_bidirectional(ctx, info1.edge_index);

    // For same-direction on single-track, check time overlap (block violation)
    if same_direction && is_single_track {
        // Check if time ranges overlap
        let time_overlap =
            segment1.time_start < segment2.time_end && segment2.time_start < segment1.time_end;

        if time_overlap {
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
                let elapsed_secs = f64::from(elapsed.num_seconds() as i32);
                let elapsed_subsec_ms = f64::from((elapsed.num_milliseconds() % 1000) as i32);
                let duration_secs = f64::from(duration.num_seconds() as i32);
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
                journey1_id: journey1.line_id.clone(),
                journey2_id: journey2.line_id.clone(),
                conflict_type: ConflictType::BlockViolation,
                segment1_times: Some((segment1.time_start, segment1.time_end)),
                segment2_times: Some((segment2.time_start, segment2.time_end)),
                platform_idx: None,
            });
        }
        return;
    }

    // For all other cases, calculate geometric intersection
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
        return;
    };

    // Check if crossing happens very close to a station
    if is_near_station(&intersection, segment1, segment2, ctx.station_margin) {
        // This is a successful station crossing - add it to the list
        let station_idx = find_nearest_station(&intersection, segment1, segment2);
        results.station_crossings.push(StationCrossing {
            time: intersection.time,
            station_idx,
            journey1_id: journey1.line_id.clone(),
            journey2_id: journey2.line_id.clone(),
        });
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
        journey1_id: journey1.line_id.clone(),
        journey2_id: journey2.line_id.clone(),
        conflict_type,
        segment1_times: None,
        segment2_times: None,
        platform_idx: None,
    });
}

/// Find segment info (`edge_index`, `track_index`) for a journey segment
fn find_journey_segment_info<'a>(
    journey: &'a TrainJourney,
    idx_start: usize,
    idx_end: usize,
    ctx: &ConflictContext,
) -> Option<&'a crate::train_journey::JourneySegment> {
    // idx_start and idx_end are global station indices from station_indices HashMap
    // We need to find which segment in this journey connects those two stations

    for (i, _) in journey.station_times.iter().enumerate().skip(1) {
        if i - 1 < journey.segments.len() {
            // Get the station names at positions i-1 and i in this journey
            let station1_name = &journey.station_times[i - 1].0;
            let station2_name = &journey.station_times[i].0;

            // Look up their global indices
            if let (Some(&s1_idx), Some(&s2_idx)) = (
                ctx.station_indices.get(station1_name.as_str()),
                ctx.station_indices.get(station2_name.as_str()),
            ) {
                // Check if this segment matches (must match exact direction)
                if idx_start == s1_idx && idx_end == s2_idx {
                    return Some(&journey.segments[i - 1]);
                }
            }
        }
    }

    None
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
    // Convert via i32 to avoid precision loss (station counts are always small)
    let y1_start = f64::from(s1_start as i32);
    let y1_end = f64::from(s1_end as i32);

    let x2_start = time_to_fraction(t2_start);
    let x2_end = time_to_fraction(t2_end);
    let y2_start = f64::from(s2_start as i32);
    let y2_end = f64::from(s2_end as i32);

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
    let buffer = chrono::Duration::minutes(PLATFORM_BUFFER_MINUTES);

    for (i, (station_name, arrival_time, departure_time)) in
        journey.station_times.iter().enumerate()
    {
        let Some(&station_idx) = ctx.station_indices.get(station_name.as_str()) else {
            continue;
        };

        // Determine which platform(s) this journey uses at this station
        let mut platforms = Vec::new();

        // If not the first station, the train arrives on the destination_platform of the previous segment
        if i > 0 && i - 1 < journey.segments.len() {
            platforms.push(journey.segments[i - 1].destination_platform);
        }

        // If not the last station, the train departs from the origin_platform of the next segment
        if i < journey.segments.len() {
            let departure_platform = journey.segments[i].origin_platform;
            // Only add if different from arrival platform (to avoid duplicates)
            if platforms.is_empty() || platforms[0] != departure_platform {
                platforms.push(departure_platform);
            }
        }

        // For each platform used, create an occupancy with buffer times
        for &platform_idx in &platforms {
            occupancies.push(PlatformOccupancy {
                station_idx,
                platform_idx,
                time_start: *arrival_time - buffer,
                time_end: *departure_time + buffer,
            });
        }
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
    let occupancies1 = extract_platform_occupancies(journey1, ctx);
    let occupancies2 = extract_platform_occupancies(journey2, ctx);

    for occ1 in &occupancies1 {
        for occ2 in &occupancies2 {
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
                    journey1_id: journey1.line_id.clone(),
                    journey2_id: journey2.line_id.clone(),
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
}
