use chrono::NaiveDateTime;
use super::{SegmentState, TrainJourney};
use crate::time::time_to_fraction;
use crate::constants::BASE_DATE;
use std::collections::HashMap;

// Conflict detection constants
const STATION_MARGIN_MINUTES: i64 = 1;
const MAX_CONFLICTS: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub time: NaiveDateTime,
    pub position: f64, // Position between stations (0.0 to 1.0)
    pub station1_idx: usize,
    pub station2_idx: usize,
    pub journey1_id: String,
    pub journey2_id: String,
    pub is_overtaking: bool, // True for same-direction conflicts, false for crossing
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationCrossing {
    pub time: NaiveDateTime,
    pub station_idx: usize,
    pub journey1_id: String,
    pub journey2_id: String,
}

struct JourneySegment {
    time_start: NaiveDateTime,
    time_end: NaiveDateTime,
    idx_start: usize,
    idx_end: usize,
}

struct ConflictContext<'a> {
    station_indices: HashMap<&'a str, usize>,
    segment_state: &'a SegmentState,
    station_margin: chrono::Duration,
}

pub fn detect_line_conflicts(
    train_journeys: &[TrainJourney],
    stations: &[String],
    segment_state: &SegmentState,
) -> (Vec<Conflict>, Vec<StationCrossing>) {
    let mut conflicts = Vec::new();
    let mut station_crossings = Vec::new();

    // Pre-compute station name to index mapping for O(1) lookups
    let station_indices: HashMap<&str, usize> = stations
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.as_str(), idx))
        .collect();

    let ctx = ConflictContext {
        station_indices,
        segment_state,
        station_margin: chrono::Duration::minutes(STATION_MARGIN_MINUTES),
    };

    // Compare each pair of journeys
    for (i, journey1) in train_journeys.iter().enumerate() {
        if conflicts.len() >= MAX_CONFLICTS {
            break;
        }
        for journey2 in train_journeys.iter().skip(i + 1) {
            check_journey_pair(journey1, journey2, &ctx, &mut conflicts, &mut station_crossings);
            if conflicts.len() >= MAX_CONFLICTS {
                break;
            }
        }
    }

    (conflicts, station_crossings)
}

fn check_journey_pair(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    conflicts: &mut Vec<Conflict>,
    station_crossings: &mut Vec<StationCrossing>,
) {
    let mut prev1: Option<(NaiveDateTime, usize)> = None;

    for (station1, time1) in &journey1.station_times {
        let Some(&station1_idx) = ctx.station_indices.get(station1.as_str()) else {
            continue;
        };

        if let Some((prev_time1, prev_idx1)) = prev1 {
            let segment1 = JourneySegment {
                time_start: prev_time1,
                time_end: *time1,
                idx_start: prev_idx1,
                idx_end: station1_idx,
            };
            check_segment_against_journey(&segment1, journey1, journey2, ctx, conflicts, station_crossings);
        }
        prev1 = Some((*time1, station1_idx));
    }
}

fn check_segment_against_journey(
    segment1: &JourneySegment,
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    conflicts: &mut Vec<Conflict>,
    station_crossings: &mut Vec<StationCrossing>,
) {
    let seg1_min = segment1.idx_start.min(segment1.idx_end);
    let seg1_max = segment1.idx_start.max(segment1.idx_end);

    let mut prev2: Option<(NaiveDateTime, usize)> = None;

    for (station2, time2) in &journey2.station_times {
        let Some(&station2_idx) = ctx.station_indices.get(station2.as_str()) else {
            continue;
        };

        if let Some((prev_time2, prev_idx2)) = prev2 {
            let segment2 = JourneySegment {
                time_start: prev_time2,
                time_end: *time2,
                idx_start: prev_idx2,
                idx_end: station2_idx,
            };

            check_segment_pair(
                segment1,
                &segment2,
                seg1_min,
                seg1_max,
                journey1,
                journey2,
                ctx,
                conflicts,
                station_crossings,
            );
            if conflicts.len() >= MAX_CONFLICTS {
                return;
            }
        }
        prev2 = Some((*time2, station2_idx));
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
    conflicts: &mut Vec<Conflict>,
    station_crossings: &mut Vec<StationCrossing>,
) {
    // Check if the segments overlap in space
    let seg2_min = segment2.idx_start.min(segment2.idx_end);
    let seg2_max = segment2.idx_start.max(segment2.idx_end);

    if seg1_max <= seg2_min || seg2_max <= seg1_min {
        return;
    }

    // Calculate intersection point
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
        station_crossings.push(StationCrossing {
            time: intersection.time,
            station_idx,
            journey1_id: journey1.line_id.clone(),
            journey2_id: journey2.line_id.clone(),
        });
        return;
    }

    // Determine if it's an overtaking or crossing conflict
    let is_overtaking = (segment1.idx_start < segment1.idx_end && segment2.idx_start < segment2.idx_end)
        || (segment1.idx_start > segment1.idx_end && segment2.idx_start > segment2.idx_end);

    // Double-tracked segments only prevent crossing conflicts, not overtaking
    if !is_overtaking {
        let segment_idx = seg1_max;
        if ctx.segment_state.double_tracked_segments.contains(&segment_idx) {
            return;
        }
    }

    conflicts.push(Conflict {
        time: intersection.time,
        position: intersection.position,
        station1_idx: seg1_min,
        station2_idx: seg1_max,
        journey1_id: journey1.line_id.clone(),
        journey2_id: journey2.line_id.clone(),
        is_overtaking,
    });
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

    times.iter().any(|t| {
        (*t - intersection.time).abs() < station_margin
    })
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
        .map(|(_, idx)| *idx)
        .unwrap_or(segment1.idx_start)
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
    let y1_start = s1_start as f64;
    let y1_end = s1_end as f64;

    let x2_start = time_to_fraction(t2_start);
    let x2_end = time_to_fraction(t2_end);
    let y2_start = s2_start as f64;
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

