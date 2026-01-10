//! Geometric intersection calculations for conflict detection.

use crate::constants::BASE_DATE;
use crate::time::time_to_fraction;
use chrono::NaiveDateTime;

#[derive(Debug)]
pub(super) struct Intersection {
    pub time: NaiveDateTime,
    pub position: f64, // Position between stations (0.0 to 1.0)
}

/// Check if an intersection is near any station (within margin)
pub(super) fn is_near_station(
    intersection: &Intersection,
    times: &[NaiveDateTime; 4],
    station_margin: chrono::Duration,
) -> bool {
    times
        .iter()
        .any(|t| (*t - intersection.time).abs() < station_margin)
}

/// Find the station index nearest to the intersection time
pub(super) fn find_nearest_station(
    intersection: &Intersection,
    times_with_idx: &[(NaiveDateTime, usize); 4],
    default_idx: usize,
) -> usize {
    times_with_idx
        .iter()
        .min_by_key(|(t, _)| (*t - intersection.time).abs())
        .map_or(default_idx, |(_, idx)| *idx)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn calculate_intersection(
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
