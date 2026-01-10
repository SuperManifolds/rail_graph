//! Platform occupancy and conflict detection.

use super::{ConflictContext, ConflictResults, MAX_CONFLICTS};
use crate::constants::BASE_MIDNIGHT;
use crate::train_journey::TrainJourney;
use crate::conflict::types::{Conflict, ConflictType};
use chrono::NaiveDateTime;

#[cfg(not(target_arch = "wasm32"))]
use super::timing;

pub(super) struct PlatformOccupancy {
    pub station_idx: usize,
    pub platform_idx: usize,
    pub time_start: NaiveDateTime,
    pub time_end: NaiveDateTime,
    pub timing_uncertain: bool,
    pub arrival_edge_index: Option<usize>,
    // Actual arrival/departure without buffer (for visualization)
    pub actual_arrival: NaiveDateTime,
    pub actual_departure: NaiveDateTime,
}

/// Extract all platform occupancies from a journey
pub(super) fn extract_platform_occupancies(
    journey: &TrainJourney,
    ctx: &ConflictContext,
) -> Vec<PlatformOccupancy> {
    let mut occupancies = Vec::new();
    let buffer = ctx.minimum_separation;

    for (i, (node_idx, arrival_time, departure_time)) in
        journey.station_times.iter().enumerate()
    {
        let Some(&station_idx) = ctx.station_indices.get(node_idx) else {
            continue;
        };

        // Skip junctions - they don't have platforms
        if ctx.serializable_ctx.junctions.contains(&node_idx.index()) {
            continue;
        }

        // Determine which platform this journey uses at this station
        // A train can only occupy ONE platform at a time during a stop
        // Priority: use arrival platform (where train stops), or departure platform if no arrival
        let (platform_idx, arrival_edge_index) = if i > 0 && i - 1 < journey.segments.len() {
            // Not the first station: use the destination platform of the previous segment (arrival platform)
            // and capture the edge index the train arrived on
            (journey.segments[i - 1].destination_platform, Some(journey.segments[i - 1].edge_index))
        } else if i < journey.segments.len() {
            // First station: use the origin platform of the current segment (departure platform)
            // No arrival edge since this is the origin
            (journey.segments[i].origin_platform, None)
        } else {
            // Single station (no segments) - use platform 0, no arrival edge
            (0, None)
        };

        // Determine if this is the first or last station in the journey
        let is_first_station = i == 0;
        let is_last_station = i == journey.station_times.len() - 1;

        // Apply buffer only when NOT at journey start/end
        // - First station (journey start): no buffer before arrival
        // - Last station (journey end): no buffer after departure
        // - Middle stations: buffer on both sides
        let time_start = if is_first_station {
            *arrival_time
        } else {
            *arrival_time - buffer
        };

        let time_end = if is_last_station {
            *departure_time
        } else {
            *departure_time + buffer
        };

        occupancies.push(PlatformOccupancy {
            station_idx,
            platform_idx,
            time_start,
            time_end,
            timing_uncertain: journey.timing_inherited.get(i).copied().unwrap_or(false),
            arrival_edge_index,
            actual_arrival: *arrival_time,
            actual_departure: *departure_time,
        });
    }

    occupancies
}

/// Check for platform conflicts using pre-cached occupancies
pub(super) fn check_platform_conflicts_cached(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    results: &mut ConflictResults,
    occupancies1: &[PlatformOccupancy],
    occupancies2: &[PlatformOccupancy],
    ctx: &ConflictContext,
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

                // Skip conflicts that occur before the week start (day -1 Sunday)
                if conflict_time < BASE_MIDNIGHT {
                    continue;
                }

                // If setting is enabled, skip conflicts where trains arrived from the same direction
                let same_direction = matches!((occ1.arrival_edge_index, occ2.arrival_edge_index), (Some(e1), Some(e2)) if e1 == e2);
                if ctx.ignore_same_direction_platform_conflicts && same_direction {
                    continue;
                }

                let timing_uncertain = occ1.timing_uncertain || occ2.timing_uncertain;

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
                    edge_index: None, // Platform conflicts don't involve edges
                    timing_uncertain,
                    actual1_times: Some((occ1.actual_arrival, occ1.actual_departure)),
                    actual2_times: Some((occ2.actual_arrival, occ2.actual_departure)),
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
