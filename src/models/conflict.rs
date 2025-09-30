use chrono::NaiveDateTime;
use super::{SegmentState, TrainJourney};
use crate::time::time_to_fraction;
use crate::constants::BASE_DATE;

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

pub fn detect_line_conflicts(
    train_journeys: &[TrainJourney],
    stations: &[String],
    segment_state: &SegmentState,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    let station_margin = chrono::Duration::minutes(STATION_MARGIN_MINUTES);

    // Compare each pair of journeys
    for (i, journey1) in train_journeys.iter().enumerate() {
        if conflicts.len() >= MAX_CONFLICTS {
            break;
        }
        for journey2 in train_journeys.iter().skip(i + 1) {
            if conflicts.len() >= MAX_CONFLICTS {
                break;
            }

            // For each pair of consecutive stations in journey1
            let mut prev1: Option<(&String, NaiveDateTime, usize)> = None;
            for (station1, time1) in &journey1.station_times {
                if let Some(station1_idx) = stations.iter().position(|s| s == station1) {
                    if let Some((_prev_station1, prev_time1, prev_idx1)) = prev1 {
                        // Check if this segment is double-tracked
                        let segment_idx = station1_idx.max(prev_idx1);
                        let is_double_tracked = segment_state
                            .double_tracked_segments
                            .contains(&segment_idx);

                        if !is_double_tracked {
                            // For each pair of consecutive stations in journey2
                            let mut prev2: Option<(&String, NaiveDateTime, usize)> = None;
                            for (station2, time2) in &journey2.station_times {
                                if let Some(station2_idx) = stations.iter().position(|s| s == station2)
                                {
                                    if let Some((_prev_station2, prev_time2, prev_idx2)) = prev2 {
                                        // Check if the segments overlap in space
                                        let seg1_min = prev_idx1.min(station1_idx);
                                        let seg1_max = prev_idx1.max(station1_idx);
                                        let seg2_min = prev_idx2.min(station2_idx);
                                        let seg2_max = prev_idx2.max(station2_idx);

                                        if seg1_max > seg2_min && seg2_max > seg1_min {
                                            // Calculate intersection point
                                            if let Some(intersection) = calculate_intersection(
                                                prev_time1,
                                                *time1,
                                                prev_idx1,
                                                station1_idx,
                                                prev_time2,
                                                *time2,
                                                prev_idx2,
                                                station2_idx,
                                            ) {
                                                // Don't count conflicts that happen very close to stations
                                                let is_near_station = journey1
                                                    .station_times
                                                    .iter()
                                                    .any(|(_, t)| {
                                                        (*t - intersection.time).abs()
                                                            < station_margin
                                                    })
                                                    || journey2.station_times.iter().any(
                                                        |(_, t)| {
                                                            (*t - intersection.time).abs()
                                                                < station_margin
                                                        },
                                                    );

                                                if !is_near_station {
                                                    // Determine if it's an overtaking or crossing conflict
                                                    let is_overtaking = (prev_idx1 < station1_idx
                                                        && prev_idx2 < station2_idx)
                                                        || (prev_idx1 > station1_idx
                                                            && prev_idx2 > station2_idx);

                                                    conflicts.push(Conflict {
                                                        time: intersection.time,
                                                        position: intersection.position,
                                                        station1_idx: seg1_min,
                                                        station2_idx: seg1_max,
                                                        journey1_id: journey1.line_id.clone(),
                                                        journey2_id: journey2.line_id.clone(),
                                                        is_overtaking,
                                                    });

                                                    if conflicts.len() >= MAX_CONFLICTS {
                                                        return conflicts;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    prev2 = Some((station2, *time2, station2_idx));
                                }
                            }
                        }
                    }
                    prev1 = Some((station1, *time1, station1_idx));
                }
            }
        }
    }

    conflicts
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

