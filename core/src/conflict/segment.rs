//! Segment conflict detection for track-based conflicts.

use super::{geometry, ConflictContext, ConflictResults, JourneySegment, MAX_CONFLICTS};
use crate::conflict::types::{Conflict, ConflictType, StationCrossing};
use crate::constants::BASE_MIDNIGHT;
use crate::train_journey::TrainJourney;
use chrono::NaiveDateTime;

#[cfg(not(target_arch = "wasm32"))]
use super::timing;

/// Segment with pre-computed spatial bounds and edge info for faster checking
#[derive(Debug, Clone, Copy)]
pub(super) struct CachedSegment {
    pub segment: JourneySegment,
    pub idx_min: usize,
    pub idx_max: usize,
    pub edge_index: usize,
    pub track_index: usize,
    pub segment_idx: usize, // Index in journey.segments array for timing checks
}

/// Build a list of journey segments with resolved station indices and pre-computed bounds
pub(super) fn build_segment_list_with_bounds(journey: &TrainJourney, ctx: &ConflictContext) -> Vec<CachedSegment> {
    let mut segments = Vec::new();
    let mut prev: Option<(NaiveDateTime, usize)> = None;
    let mut segment_idx = 0;

    for (node_idx, arrival_time, departure_time) in &journey.station_times {
        let Some(&station_idx) = ctx.station_indices.get(node_idx) else {
            continue;
        };

        if let Some((prev_departure_time, prev_idx)) = prev {
            // Get edge and track info from the journey segment
            let (edge_index, track_index) = if segment_idx < journey.segments.len() {
                let seg_info = &journey.segments[segment_idx];
                (seg_info.edge_index, seg_info.track_index)
            } else {
                (0, 0) // Fallback, should not happen in valid data
            };

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
                edge_index,
                track_index,
                segment_idx,
            });
            segment_idx += 1;
        }
        prev = Some((*departure_time, station_idx));
    }

    segments
}

#[allow(clippy::similar_names)]
pub(super) fn check_segments_for_pair_cached(
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
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
            let seg2 = &cached2.segment;

            // If seg1 ends before seg2 starts, no more overlaps possible
            if seg1.time_end < seg2.time_start {
                break;
            }

            // Skip segments that are entirely before the week start (day -1 Sunday)
            // Only process conflicts that could occur during the current week
            if seg1.time_end < BASE_MIDNIGHT && seg2.time_end < BASE_MIDNIGHT {
                continue;
            }

            // Quick spatial overlap check before calling expensive function
            // This filters out ~50% of segment pairs that don't spatially overlap
            if cached1.idx_max <= cached2.idx_min || cached2.idx_max <= cached1.idx_min {
                continue;
            }

            // Early edge/track filtering - most segments are on different edges
            // Check if they're on the same edge OR reverse edges
            let same_edge = cached1.edge_index == cached2.edge_index;
            let reverse_edges = are_reverse_bidirectional_edges(
                ctx,
                cached1.edge_index,
                cached2.edge_index,
                cached1.track_index,
                cached2.track_index,
                (seg1.idx_start, seg1.idx_end),
                (seg2.idx_start, seg2.idx_end),
            );

            if !same_edge && !reverse_edges {
                continue; // Different edges, no conflict possible
            }

            // If same edge, must be same track
            if same_edge && cached1.track_index != cached2.track_index {
                continue; // Different tracks on same edge, no conflict
            }

            check_segment_pair(
                seg1, seg2, cached1.idx_min, cached1.idx_max, cached1.edge_index,
                journey1, journey2, cached1.segment_idx, cached2.segment_idx, ctx, results,
            );

            if results.conflicts.len() >= MAX_CONFLICTS {
                return;
            }
        }
    }
}

/// Check if timing is uncertain for a segment using direct indexing (O(1))
/// For a segment at index `seg_idx`, the destination station is at `station_times[seg_idx + 1]`
fn has_inherited_timing_at_segment(journey: &TrainJourney, seg_idx: usize) -> bool {
    // The destination station of segment[seg_idx] is at station_times[seg_idx + 1]
    journey.timing_inherited.get(seg_idx + 1).copied().unwrap_or(false)
}

#[allow(clippy::too_many_arguments, clippy::similar_names, clippy::too_many_lines)]
fn check_segment_pair(
    segment1: &JourneySegment,
    segment2: &JourneySegment,
    seg1_min: usize,
    seg1_max: usize,
    edge_index: usize,
    journey1: &TrainJourney,
    journey2: &TrainJourney,
    seg1_idx: usize,
    seg2_idx: usize,
    ctx: &ConflictContext,
    results: &mut ConflictResults,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::Ordering;
        timing::SEGMENT_PAIR_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    // Determine travel directions
    let same_direction = (segment1.idx_start < segment1.idx_end
        && segment2.idx_start < segment2.idx_end)
        || (segment1.idx_start > segment1.idx_end && segment2.idx_start > segment2.idx_end);

    let is_single_track = is_single_track_bidirectional(ctx, edge_index);

    // For same-direction on single-track, check time overlap (block violation)
    if same_direction && is_single_track {
        // Check if time ranges overlap
        let time_overlap =
            segment1.time_start < segment2.time_end && segment2.time_start < segment1.time_end;

        if time_overlap {
            // Two trains on same single-track block at same time, same direction = block violation
            // Conflict occurs when the trailing train enters while leading train is still in block
            let conflict_time = segment1.time_start.max(segment2.time_start);

            // Skip conflicts that occur before the week start (day -1 Sunday)
            if conflict_time < BASE_MIDNIGHT {
                return;
            }

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

            let timing_uncertain = has_inherited_timing_at_segment(journey1, seg1_idx)
                || has_inherited_timing_at_segment(journey2, seg2_idx);

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
                edge_index: Some(edge_index),
                timing_uncertain,
                actual1_times: None,
                actual2_times: None,
            });
        }

        return;
    }

    // For all other cases, calculate geometric intersection
    #[cfg(not(target_arch = "wasm32"))]
    let intersection_start = std::time::Instant::now();

    let Some(intersection) = geometry::calculate_intersection(
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

    #[cfg(not(target_arch = "wasm32"))]
    timing::add_duration(&timing::INTERSECTION_TIME, intersection_start.elapsed());

    // Check if crossing happens very close to a station
    let station_times = [
        segment1.time_start,
        segment1.time_end,
        segment2.time_start,
        segment2.time_end,
    ];
    if geometry::is_near_station(&intersection, &station_times, ctx.station_margin) {
        // This is a successful station crossing - add it to the list (if in current week)
        // Skip crossings that occur before the week start (day -1 Sunday)
        if intersection.time >= BASE_MIDNIGHT {
            let times_with_idx = [
                (segment1.time_start, segment1.idx_start),
                (segment1.time_end, segment1.idx_end),
                (segment2.time_start, segment2.idx_start),
                (segment2.time_end, segment2.idx_end),
            ];
            let station_idx = geometry::find_nearest_station(&intersection, &times_with_idx, segment1.idx_start);
            results.station_crossings.push(StationCrossing {
                time: intersection.time,
                station_idx,
                journey1_id: journey1.train_number.clone(),
                journey2_id: journey2.train_number.clone(),
            });
        }

        return;
    }

    // Skip conflicts that occur before the week start (day -1 Sunday)
    if intersection.time < BASE_MIDNIGHT {
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

    // Store segment timing for all conflict types (BlockViolation, HeadOn, Overtaking).
    // This enables block visualization when hovering over any conflict, showing
    // the time ranges when each train occupied the conflicting track segment.
    let timing_uncertain = has_inherited_timing_at_segment(journey1, seg1_idx)
        || has_inherited_timing_at_segment(journey2, seg2_idx);

    results.conflicts.push(Conflict {
        time: intersection.time,
        position: intersection.position,
        station1_idx: seg1_min,
        station2_idx: seg1_max,
        journey1_id: journey1.train_number.clone(),
        journey2_id: journey2.train_number.clone(),
        conflict_type,
        segment1_times: Some((segment1.time_start, segment1.time_end)),
        segment2_times: Some((segment2.time_start, segment2.time_end)),
        platform_idx: None,
        edge_index: Some(edge_index),
        timing_uncertain,
        actual1_times: None,
        actual2_times: None,
    });
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

    // Check if both tracks are bidirectional (single-track case)
    let edge1_bidir = ctx.serializable_ctx.track_directions
        .get(&(edge1_index, track1_index))
        .copied()
        .unwrap_or(false);

    let edge2_bidir = ctx.serializable_ctx.track_directions
        .get(&(edge2_index, track2_index))
        .copied()
        .unwrap_or(false);

    edge1_bidir && edge2_bidir
}

/// Check if an edge has only 1 bidirectional track (single-track section)
pub(super) fn is_single_track_bidirectional(ctx: &ConflictContext, edge_index: usize) -> bool {
    ctx.serializable_ctx.edge_info
        .get(&edge_index)
        .is_some_and(|&(is_single_bi, _)| is_single_bi)
}
