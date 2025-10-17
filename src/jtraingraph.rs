use serde::Deserialize;
use crate::models::{RailwayGraph, Line, RouteSegment, ManualDeparture, ScheduleMode, DaysOfWeek, Track, TrackDirection, Stations, Tracks, generate_random_color};
use crate::constants::BASE_DATE;
use chrono::{Duration, NaiveTime, Timelike};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use std::collections::HashMap;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "jTrainGraph_timetable")]
#[allow(clippy::struct_excessive_bools)]
pub struct JTrainGraphTimetable {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@tMin")]
    pub t_min: String,
    #[serde(rename = "@tMax")]
    pub t_max: String,
    #[serde(rename = "@d")]
    pub days: String,
    #[serde(rename = "@bgC")]
    pub bg_color: String,
    #[serde(rename = "@sFont")]
    pub station_font: String,
    #[serde(rename = "@trFont")]
    pub train_font: String,
    #[serde(rename = "@hFont")]
    pub header_font: String,
    #[serde(rename = "@tFont")]
    pub time_font: String,
    #[serde(rename = "@sHor")]
    pub stations_horizontal: bool,
    #[serde(rename = "@sLine")]
    pub station_line: String,
    #[serde(rename = "@shKm")]
    pub show_km: bool,
    #[serde(rename = "@sStation")]
    pub start_station: String,
    #[serde(rename = "@eStation")]
    pub end_station: String,
    #[serde(rename = "@cNr")]
    pub column_number: String,
    #[serde(rename = "@exW")]
    pub extra_width: String,
    #[serde(rename = "@hpH")]
    pub hour_pixel_height: String,
    #[serde(rename = "@shV")]
    pub show_vertical: String,
    #[serde(rename = "@shT")]
    pub show_time: bool,
    #[serde(rename = "@shC")]
    pub show_color: bool,
    #[serde(rename = "@hlI")]
    pub highlight_interval: String,
    #[serde(rename = "@hlC")]
    pub highlight_color: String,
    #[serde(rename = "@p")]
    pub print: bool,
    #[serde(rename = "@pC")]
    pub print_columns: String,
    #[serde(rename = "@mpP")]
    pub min_print_pause: String,
    #[serde(rename = "@rT")]
    pub round_time: bool,
    #[serde(rename = "@shMu")]
    pub show_multi: bool,
    #[serde(rename = "@dTt")]
    pub default_travel_time: String,
    #[serde(rename = "@odBT")]
    pub override_default_brake_time: String,
    #[serde(rename = "@isTV")]
    pub is_tv: bool,
    pub stations: JTrainGraphStations,
    pub trains: JTrainGraphTrains,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct JTrainGraphStations {
    #[serde(rename = "sta", default)]
    pub stations: Vec<JTrainGraphStation>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct JTrainGraphStation {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@kml")]
    pub km_left: String,
    #[serde(rename = "@kmr")]
    pub km_right: String,
    #[serde(rename = "@cl")]
    pub color: String,
    #[serde(rename = "@sh")]
    pub show: bool,
    #[serde(rename = "@sz")]
    pub size: String,
    #[serde(rename = "@sy")]
    pub symbol: String,
    #[serde(rename = "@sri")]
    pub show_route_in: bool,
    #[serde(rename = "@sra")]
    pub show_route_all: bool,
    #[serde(rename = "@tr")]
    pub tracks: String,
    #[serde(rename = "@dTi")]
    pub default_platform_in: String,
    #[serde(rename = "@dTa")]
    pub default_platform_away: String,
    #[serde(rename = "track", default)]
    pub platforms: Vec<JTrainGraphPlatform>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct JTrainGraphPlatform {
    #[serde(rename = "@name")]
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct JTrainGraphTrains {
    #[serde(rename = "ti", default)]
    pub trains_in: Vec<JTrainGraphTrainInfo>,
    #[serde(rename = "ta", default)]
    pub trains_away: Vec<JTrainGraphTrainInfo>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct JTrainGraphTrainInfo {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@cm")]
    pub comment: String,
    #[serde(rename = "@cl")]
    pub color: String,
    #[serde(rename = "@sh")]
    pub show: bool,
    #[serde(rename = "@sz")]
    pub size: String,
    #[serde(rename = "@sy")]
    pub symbol: String,
    #[serde(rename = "@d")]
    pub days: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "t", default)]
    pub times: Vec<JTrainGraphTrainTime>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct JTrainGraphTrainTime {
    #[serde(rename = "@a", default)]
    pub arrival: String,
    #[serde(rename = "@d", default)]
    pub departure: String,
    #[serde(rename = "@at", default)]
    pub arrival_track: String,
    #[serde(rename = "@dt", default)]
    pub departure_track: String,
}

/// Parse `JTrainGraph` XML content into timetable structure
///
/// # Errors
/// Returns error if XML parsing fails
pub fn parse_jtraingraph(xml_content: &str) -> Result<JTrainGraphTimetable, quick_xml::DeError> {
    quick_xml::de::from_str(xml_content)
}

/// Parse `JTrainGraph` time format (HH:MM or HH:MM:SS) to `NaiveTime`
fn parse_time(time_str: &str) -> Option<NaiveTime> {
    if time_str.is_empty() {
        return None;
    }

    let parts: Vec<&str> = time_str.split(':').collect();

    match parts.len() {
        2 => {
            // HH:MM format
            let hour = parts[0].parse::<u32>().ok()?;
            let minute = parts[1].parse::<u32>().ok()?;
            NaiveTime::from_hms_opt(hour, minute, 0)
        }
        3 => {
            // HH:MM:SS format
            let hour = parts[0].parse::<u32>().ok()?;
            let minute = parts[1].parse::<u32>().ok()?;
            let second = parts[2].parse::<u32>().ok()?;
            NaiveTime::from_hms_opt(hour, minute, second)
        }
        _ => None,
    }
}

/// Represents a stop pattern for grouping trains (includes durations and platforms)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StopPattern {
    /// Indices of stations where the train stops
    station_indices: Vec<usize>,
    /// Duration between each pair of consecutive stops (in seconds)
    durations: Vec<i64>,
    /// Platform names at each stop (`arrival_track`, `departure_track`)
    platforms: Vec<(String, String)>,
}

/// Key for grouping trains by stop pattern and direction
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PatternKey {
    pattern: StopPattern,
    is_return: bool,
}

/// Extract the stop pattern from a train (including durations and platforms)
/// For return trains (`is_return=true`), reverses the order to treat them as forward journeys
fn get_stop_pattern(train: &JTrainGraphTrainInfo, is_return: bool) -> Option<StopPattern> {
    let mut station_indices = Vec::new();
    let mut durations = Vec::new();
    let mut platforms = Vec::new();

    // Collect stops
    for (idx, time) in train.times.iter().enumerate() {
        if !time.arrival.is_empty() || !time.departure.is_empty() {
            station_indices.push(idx);
            // Store platform names (arrival_track, departure_track)
            platforms.push((time.arrival_track.clone(), time.departure_track.clone()));
        }
    }

    if station_indices.len() < 2 {
        return None;
    }

    // For return trains, swap arrival/departure platforms but keep station_indices in original order
    // This allows route creation to still work with ascending indices
    if is_return {
        platforms = platforms.into_iter()
            .map(|(at, dt)| (dt, at)) // Swap arrival and departure
            .collect();
    }

    // Calculate durations between consecutive stops
    // For return trains, we iterate backwards through the stops
    let stop_pairs: Vec<(usize, usize)> = if is_return {
        // For return: go from last stop to first stop (e.g., [48,44], [44,43], ...)
        (0..station_indices.len() - 1).rev()
            .map(|i| (station_indices[i + 1], station_indices[i]))
            .collect()
    } else {
        // For forward: go from first stop to last stop
        (0..station_indices.len() - 1)
            .map(|i| (station_indices[i], station_indices[i + 1]))
            .collect()
    };

    for (from_idx, to_idx) in stop_pairs {
        let from_time = &train.times[from_idx];
        let to_time = &train.times[to_idx];

        // Times in the array are always the real-world times, regardless of direction
        // Use departure time from origin station
        let dep_time = parse_time(&from_time.departure)?;

        // Use arrival time if available at destination, otherwise use departure time
        let arr_time_str = if to_time.arrival.is_empty() {
            &to_time.departure
        } else {
            &to_time.arrival
        };
        let arr_time = parse_time(arr_time_str)?;

        let dep_seconds = i64::from(dep_time.num_seconds_from_midnight());
        let arr_seconds = i64::from(arr_time.num_seconds_from_midnight());

        let duration = if arr_seconds >= dep_seconds {
            arr_seconds - dep_seconds
        } else {
            // Crossed midnight
            86400 - dep_seconds + arr_seconds
        };

        durations.push(duration);
    }

    Some(StopPattern {
        station_indices,
        durations,
        platforms,
    })
}

/// Calculate wait time at a destination station and insert into map
fn calculate_wait_time(
    trains: &[&JTrainGraphTrainInfo],
    to_idx: usize,
    wait_time_map: &mut HashMap<usize, i64>,
) {
    let Some(first_train) = trains.first() else {
        return;
    };

    let dest_time = &first_train.times[to_idx];
    if dest_time.arrival.is_empty() || dest_time.departure.is_empty() {
        return;
    }

    let Some((arr_time, dep_time)) =
        parse_time(&dest_time.arrival).zip(parse_time(&dest_time.departure)) else {
        return;
    };

    let arr_seconds = i64::from(arr_time.num_seconds_from_midnight());
    let dep_seconds = i64::from(dep_time.num_seconds_from_midnight());
    let wait_seconds = if dep_seconds >= arr_seconds {
        dep_seconds - arr_seconds
    } else {
        86400 - arr_seconds + dep_seconds
    };
    wait_time_map.insert(to_idx, wait_seconds);
}

/// Add platforms to a station node, replacing any existing platforms
fn add_platforms_to_station(
    graph: &mut RailwayGraph,
    node_idx: NodeIndex,
    platforms: &[JTrainGraphPlatform],
) {
    if platforms.is_empty() {
        return;
    }

    let Some(node) = graph.graph.node_weight_mut(node_idx) else {
        return;
    };

    let Some(station_node) = node.as_station_mut() else {
        return;
    };

    // Replace platforms with imported data
    station_node.platforms = platforms.iter()
        .map(|p| crate::models::Platform { name: p.name.clone() })
        .collect();
}

/// Find platform index by name in the platform list
/// Returns None if platform name is empty or not found
fn find_platform_index(platforms: &[JTrainGraphPlatform], platform_name: &str) -> Option<usize> {
    if platform_name.is_empty() {
        return None;
    }

    platforms.iter().position(|p| p.name == platform_name)
}

/// Create route segments for a pattern
#[allow(clippy::too_many_arguments)]
fn create_route_segments(
    pattern: &StopPattern,
    trains: &[&JTrainGraphTrainInfo],
    first_station_idx: usize,
    last_station_idx: usize,
    station_node_indices: &[NodeIndex],
    edge_map: &HashMap<(NodeIndex, NodeIndex), EdgeIndex>,
    graph: &RailwayGraph,
    is_return: bool,
) -> Result<Vec<RouteSegment>, String> {
    let mut route_segments = Vec::new();

    // Build maps for segments we have timing data for
    let mut duration_map = HashMap::new();
    let mut wait_time_map = HashMap::new();

    // For return trains, durations[0] is from last stop to second-to-last stop
    // But we need to map it to the correct station index key
    for (i, &duration_seconds) in pattern.durations.iter().enumerate() {
        let (from_idx, to_idx) = if is_return {
            // For return: durations[i] goes from station_indices[len-1-i] to station_indices[len-2-i]
            let len = pattern.station_indices.len();
            (pattern.station_indices[len - 1 - i], pattern.station_indices[len - 2 - i])
        } else {
            // For forward: durations[i] goes from station_indices[i] to station_indices[i+1]
            (pattern.station_indices[i], pattern.station_indices[i + 1])
        };

        // Set duration on the FIRST segment after departure (forward-looking inheritance)
        duration_map.insert(from_idx, duration_seconds);

        // Calculate wait time at destination station from first train in pattern
        calculate_wait_time(trains, to_idx, &mut wait_time_map);
    }

    // Build a map of station_idx -> (stop_idx in pattern) for stations where train stops
    let stop_map: HashMap<usize, usize> = pattern.station_indices.iter()
        .enumerate()
        .map(|(stop_idx, &station_idx)| (station_idx, stop_idx))
        .collect();

    // Track current platform as we traverse the route
    let mut current_platform: Option<usize> = None;

    // Create segments for all consecutive stations in the route
    // For return trains, iterate in reverse (from last to first)
    let station_pairs: Vec<(usize, usize)> = if is_return {
        (first_station_idx..last_station_idx).rev()
            .map(|i| (i + 1, i))
            .collect()
    } else {
        (first_station_idx..last_station_idx)
            .map(|i| (i, i + 1))
            .collect()
    };

    for (from_station_idx, to_station_idx) in station_pairs {
        let from_node = station_node_indices[from_station_idx];
        let to_node = station_node_indices[to_station_idx];

        // Edges are always created in ascending order (lower index to higher index)
        // For return trains, we need to look up the edge in the correct direction
        let (edge_from, edge_to, traveling_backward) = if from_station_idx < to_station_idx {
            (from_node, to_node, false)
        } else {
            (to_node, from_node, true)
        };

        let edge = edge_map.get(&(edge_from, edge_to))
            .ok_or_else(|| format!("No edge found between stations {from_station_idx} and {to_station_idx}"))?;

        let origin_platforms = graph.graph.node_weight(from_node)
            .and_then(|n| n.as_station())
            .map_or(1, |s| s.platforms.len());

        let dest_platforms = graph.graph.node_weight(to_node)
            .and_then(|n| n.as_station())
            .map_or(1, |s| s.platforms.len());

        // Determine origin platform
        let origin_platform = if let Some(stop_idx) = stop_map.get(&from_station_idx) {
            // This is a stop - use departure platform from pattern
            let (arrival_track, departure_track) = &pattern.platforms[*stop_idx];
            if !departure_track.is_empty() {
                // Explicit departure platform specified
                let station_platforms = graph.graph.node_weight(from_node)
                    .and_then(|n| n.as_station())
                    .map(|s| s.platforms.iter()
                        .map(|p| JTrainGraphPlatform { name: p.name.clone() })
                        .collect::<Vec<_>>())
                    .unwrap_or_default();
                let platform = find_platform_index(&station_platforms, departure_track)
                    .unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, false, origin_platforms));
                current_platform = Some(platform);
                platform
            } else if !arrival_track.is_empty() {
                // No departure platform but arrival platform specified - stay on arrival platform
                let station_platforms = graph.graph.node_weight(from_node)
                    .and_then(|n| n.as_station())
                    .map(|s| s.platforms.iter()
                        .map(|p| JTrainGraphPlatform { name: p.name.clone() })
                        .collect::<Vec<_>>())
                    .unwrap_or_default();
                let platform = find_platform_index(&station_platforms, arrival_track)
                    .unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, false, origin_platforms));
                current_platform = Some(platform);
                platform
            } else {
                // No platform specified, use current or default
                current_platform.unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, false, origin_platforms))
            }
        } else {
            // Pass-through station - use current platform or default
            current_platform.unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, false, origin_platforms))
        };

        // Determine destination platform
        let destination_platform = if let Some(stop_idx) = stop_map.get(&to_station_idx) {
            // This is a stop - use arrival platform from pattern
            let (arrival_track, departure_track) = &pattern.platforms[*stop_idx];
            if !arrival_track.is_empty() {
                // Explicit arrival platform specified
                let station_platforms = graph.graph.node_weight(to_node)
                    .and_then(|n| n.as_station())
                    .map(|s| s.platforms.iter()
                        .map(|p| JTrainGraphPlatform { name: p.name.clone() })
                        .collect::<Vec<_>>())
                    .unwrap_or_default();
                let platform = find_platform_index(&station_platforms, arrival_track)
                    .unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, true, dest_platforms));
                current_platform = Some(platform);
                platform
            } else if !departure_track.is_empty() {
                // No arrival platform but departure platform specified - use departure platform
                let station_platforms = graph.graph.node_weight(to_node)
                    .and_then(|n| n.as_station())
                    .map(|s| s.platforms.iter()
                        .map(|p| JTrainGraphPlatform { name: p.name.clone() })
                        .collect::<Vec<_>>())
                    .unwrap_or_default();
                let platform = find_platform_index(&station_platforms, departure_track)
                    .unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, true, dest_platforms));
                current_platform = Some(platform);
                platform
            } else {
                // No platform specified, use current or default
                current_platform.unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, true, dest_platforms))
            }
        } else {
            // Pass-through station - use current platform or default
            current_platform.unwrap_or_else(|| graph.get_default_platform_for_arrival(*edge, true, dest_platforms))
        };

        // Use duration if we have timing data starting from this station, otherwise None
        let duration = duration_map.get(&from_station_idx)
            .map(|&d| Duration::seconds(d));

        // Use wait time if we have data for the destination station, otherwise 0 (pass-through)
        let wait_time = wait_time_map.get(&to_station_idx)
            .map_or(Duration::seconds(0), |&w| Duration::seconds(w));

        // Select a track compatible with our travel direction
        let track_index = graph.graph.edge_weight(*edge)
            .and_then(|track_segment| {
                track_segment.tracks.iter().position(|t| {
                    if traveling_backward {
                        matches!(t.direction, TrackDirection::Backward | TrackDirection::Bidirectional)
                    } else {
                        matches!(t.direction, TrackDirection::Forward | TrackDirection::Bidirectional)
                    }
                })
            })
            .unwrap_or(0);

        route_segments.push(RouteSegment {
            edge_index: edge.index(),
            track_index,
            origin_platform,
            destination_platform,
            duration,
            wait_time,
        });
    }

    Ok(route_segments)
}

/// Import `JTrainGraph` timetable infrastructure (stations and tracks) and return lines to add
///
/// # Errors
/// Returns error if edge not found between stations or invalid time data
#[allow(clippy::too_many_lines)]
pub fn import_jtraingraph(
    timetable: &JTrainGraphTimetable,
    graph: &mut RailwayGraph,
    starting_line_count: usize,
    existing_line_ids: &[String],
) -> Result<Vec<Line>, String> {
    // Step 1: Create or match stations
    let station_node_indices: Vec<NodeIndex> = timetable.stations.stations
        .iter()
        .map(|station| {
            let node_idx = graph.add_or_get_station(station.name.clone());
            add_platforms_to_station(graph, node_idx, &station.platforms);
            node_idx
        })
        .collect();

    // Step 2: Create tracks between consecutive stations
    // The "tracks" property on a station indicates the number of tracks to the NEXT station
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), EdgeIndex> = HashMap::new();

    for (i, window) in station_node_indices.windows(2).enumerate() {
        let from = window[0];
        let to = window[1];

        edge_map.entry((from, to)).or_insert_with(|| {
            let from_station = &timetable.stations.stations[i];
            let to_station = &timetable.stations.stations[i + 1];

            // Parse the tracks property from the station
            let track_count = from_station.tracks
                .parse::<usize>()
                .unwrap_or(1);

            // Assign track directions based on count:
            // 1 track: Bidirectional
            // 2 tracks: Forward, Backward
            // 3 tracks: Forward, Bidirectional, Backward
            // 4 tracks: Forward, Forward, Backward, Backward
            // 5 tracks: Forward, Forward, Bidirectional, Backward, Backward
            // Pattern: outer tracks are directional, middle track(s) bidirectional for odd counts
            let tracks: Vec<Track> = (0..track_count)
                .map(|i| {
                    let direction = if track_count == 1 {
                        TrackDirection::Bidirectional
                    } else if track_count % 2 == 1 && i == track_count / 2 {
                        // Middle track in odd count is bidirectional
                        TrackDirection::Bidirectional
                    } else if i < track_count / 2 {
                        // First half: Forward
                        TrackDirection::Forward
                    } else {
                        // Second half: Backward
                        TrackDirection::Backward
                    };
                    Track { direction }
                })
                .collect();

            // Parse default platforms
            // dTa (default platform away) = platform when departing from source station
            // dTi (default platform in) = platform when arriving at destination station
            // These are platform names, we need to find their index
            let default_platform_source = find_platform_index(&from_station.platforms, &from_station.default_platform_away);
            let default_platform_target = find_platform_index(&to_station.platforms, &to_station.default_platform_in);

            let edge_idx = graph.add_track(from, to, tracks);

            // Set default platforms on the track segment
            if let Some(track_segment) = graph.graph.edge_weight_mut(edge_idx) {
                track_segment.default_platform_source = default_platform_source;
                track_segment.default_platform_target = default_platform_target;
            }

            edge_idx
        });
    }

    // Step 3: Group trains by stop pattern (including durations)
    // We need to track whether each pattern group is for return trains or not
    let mut pattern_groups: HashMap<PatternKey, Vec<&JTrainGraphTrainInfo>> = HashMap::new();

    // Process ti trains (forward direction)
    for train in &timetable.trains.trains_in {
        if let Some(pattern) = get_stop_pattern(train, false) {
            pattern_groups.entry(PatternKey { pattern, is_return: false }).or_default().push(train);
        }
    }

    // Process ta trains (return direction, reversed to forward)
    for train in &timetable.trains.trains_away {
        if let Some(pattern) = get_stop_pattern(train, true) {
            pattern_groups.entry(PatternKey { pattern, is_return: true }).or_default().push(train);
        }
    }

    // Step 4: Create lines from grouped trains
    let mut new_lines = Vec::new();

    // Sort patterns by station indices, durations, and platforms for deterministic ordering
    let mut sorted_patterns: Vec<_> = pattern_groups.into_iter().collect();
    sorted_patterns.sort_by(|(a, _), (b, _)| {
        a.pattern.station_indices.cmp(&b.pattern.station_indices)
            .then_with(|| a.pattern.durations.cmp(&b.pattern.durations))
            .then_with(|| a.pattern.platforms.cmp(&b.pattern.platforms))
            .then_with(|| a.is_return.cmp(&b.is_return))
    });

    for (pattern_idx, (pattern_key, trains)) in sorted_patterns.iter().enumerate() {
        let pattern = &pattern_key.pattern;
        let is_return = pattern_key.is_return;
        // Get first and last station indices
        let first_station_idx = pattern.station_indices[0];
        let Some(&last_station_idx) = pattern.station_indices.last() else {
            continue; // Skip if no last station (shouldn't happen)
        };

        // Get station names for line ID
        // For return trains, swap the station names so the line ID shows the actual direction of travel
        let (from_station, to_station) = if is_return {
            // Return trains travel from last_station to first_station
            (&timetable.stations.stations[last_station_idx].name,
             &timetable.stations.stations[first_station_idx].name)
        } else {
            // Forward trains travel from first_station to last_station
            (&timetable.stations.stations[first_station_idx].name,
             &timetable.stations.stations[last_station_idx].name)
        };
        let line_id = format!("{from_station} - {to_station}");

        // Build route segments for ALL consecutive stations between first and last stop
        let route_segments = create_route_segments(
            pattern,
            trains,
            first_station_idx,
            last_station_idx,
            &station_node_indices,
            &edge_map,
            graph,
            is_return,
        )?;

        // Create manual departures for each train
        let manual_departures: Vec<ManualDeparture> = trains.iter()
            .filter_map(|train| {
                // For return trains, departure is from the last station in the array (going backwards)
                // For forward trains, departure is from the first station
                let (dep_station_idx, arr_station_idx) = if is_return {
                    let last = *pattern.station_indices.last()?;
                    let first = pattern.station_indices[0];
                    (last, first)
                } else {
                    let first = pattern.station_indices[0];
                    let last = *pattern.station_indices.last()?;
                    (first, last)
                };

                // Always use the departure time at the departure station
                // The times array contains real-world times regardless of train direction
                let time_str = &train.times[dep_station_idx].departure;

                if time_str.is_empty() {
                    return None;
                }

                let Some(departure_time) = parse_time(time_str) else {
                    leptos::logging::error!("Failed to parse departure time '{}' for train '{}'",
                        time_str, train.name);
                    return None;
                };
                let departure_datetime = BASE_DATE.and_time(departure_time);

                Some(ManualDeparture {
                    id: uuid::Uuid::new_v4(),
                    time: departure_datetime,
                    from_station: station_node_indices[dep_station_idx],
                    to_station: station_node_indices[arr_station_idx],
                    days_of_week: DaysOfWeek::ALL_DAYS,
                    train_number: Some(train.name.clone()),
                })
            })
            .collect();

        if manual_departures.is_empty() {
            continue;
        }

        // Create the line
        let line = Line {
            id: if existing_line_ids.contains(&line_id) {
                format!("{line_id} ({pattern_idx})")
            } else {
                line_id
            },
            frequency: Duration::hours(1),
            color: generate_random_color(starting_line_count + pattern_idx),
            thickness: 2.0,
            first_departure: manual_departures[0].time,
            return_first_departure: BASE_DATE.and_hms_opt(0, 0, 0).ok_or_else(|| "Invalid return departure time".to_string())?,
            visible: true,
            schedule_mode: ScheduleMode::Manual,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures,
            forward_route: route_segments,
            return_route: Vec::new(),
            sync_routes: false,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(23, 59, 0).ok_or_else(|| "Invalid last departure time".to_string())?,
        };

        new_lines.push(line);
    }

    Ok(new_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_fpl() {
        let xml_content = std::fs::read_to_string("test.fpl")
            .expect("Failed to read test.fpl");

        let result = parse_jtraingraph(&xml_content);
        assert!(result.is_ok(), "Failed to parse test.fpl: {:?}", result.err());

        if let Ok(timetable) = result {
            assert_eq!(timetable.version, "012");
            assert!(!timetable.stations.stations.is_empty());
            let total_trains = timetable.trains.trains_in.len() + timetable.trains.trains_away.len();
            assert!(total_trains > 0);
        }
    }

    #[test]
    fn test_import_dortmund_fpl() {
        let xml_content = std::fs::read_to_string("dortmund.fpl")
            .expect("Failed to read dortmund.fpl");

        let timetable = parse_jtraingraph(&xml_content)
            .expect("Failed to parse dortmund.fpl");

        let mut graph = RailwayGraph::new();

        let result = import_jtraingraph(&timetable, &mut graph, 0, &[]);
        assert!(result.is_ok(), "Failed to import: {:?}", result.err());

        let lines = result.expect("Import should succeed");

        // Verify we created stations
        assert!(graph.graph.node_count() > 0, "No stations were created");

        // Verify we created lines
        assert!(!lines.is_empty(), "No lines were created");
    }

    #[test]
    fn test_import_test_fpl() {
        let xml_content = std::fs::read_to_string("test.fpl")
            .expect("Failed to read test.fpl");

        let timetable = parse_jtraingraph(&xml_content)
            .expect("Failed to parse test.fpl");

        let mut graph = RailwayGraph::new();

        let result = import_jtraingraph(&timetable, &mut graph, 0, &[]);
        assert!(result.is_ok(), "Failed to import: {:?}", result.err());

        let lines = result.expect("Import should succeed");

        // Verify we created stations
        assert!(graph.graph.node_count() > 0, "No stations were created");
        assert_eq!(graph.graph.node_count(), 32, "Expected 32 stations");

        // Verify we created lines
        assert!(!lines.is_empty(), "No lines were created");
    }

    #[test]
    fn test_platform_assignments_for_specific_trains() {
        let xml_content = std::fs::read_to_string("dortmund.fpl")
            .expect("Failed to read dortmund.fpl");

        let timetable = parse_jtraingraph(&xml_content)
            .expect("Failed to parse dortmund.fpl");

        let mut graph = RailwayGraph::new();

        let result = import_jtraingraph(&timetable, &mut graph, 0, &[]);
        assert!(result.is_ok(), "Failed to import: {:?}", result.err());

        let lines = result.expect("Import should succeed");

        // Find Gouda station index
        let gouda_idx = graph.get_all_stations_ordered()
            .iter()
            .enumerate()
            .find(|(_, (_, station))| station.name.contains("Gouda"))
            .map(|(idx, _)| idx)
            .expect("Gouda station not found");

        // Get Gouda platforms to create index mapping
        let gouda_platforms: Vec<String> = graph.get_all_stations_ordered()
            .get(gouda_idx)
            .and_then(|(node_idx, _)| graph.graph.node_weight(*node_idx))
            .and_then(|n| n.as_station())
            .map(|s| s.platforms.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();

        print_train_platform_info(&lines, &gouda_platforms);

        // Verify platform indices exist
        let platform_3_idx = gouda_platforms.iter().position(|p| p == "3");
        let platform_6_idx = gouda_platforms.iter().position(|p| p == "6");

        assert!(platform_3_idx.is_some(), "Platform '3' should exist");
        assert!(platform_6_idx.is_some(), "Platform '6' should exist");
    }

    fn print_train_platform_info(_lines: &[Line], _gouda_platforms: &[String]) {
        // Platform debugging - kept as a no-op for future debugging
    }

    #[test]
    fn test_return_train_creates_line() {
        let xml_content = std::fs::read_to_string("dortmund.fpl")
            .expect("Failed to read dortmund.fpl");

        let timetable = parse_jtraingraph(&xml_content)
            .expect("Failed to parse dortmund.fpl");

        // Verify BR 229-02 exists in ta trains
        let br_229 = timetable.trains.trains_away.iter()
            .find(|t| t.name == "BR 229-02")
            .expect("BR 229-02 not found in trains_away");

        // Get the stop pattern for BR 229-02
        let _pattern = get_stop_pattern(br_229, true)
            .expect("BR 229-02 should have a valid stop pattern");

        // Import and verify line was created
        let mut graph = RailwayGraph::new();
        let result = import_jtraingraph(&timetable, &mut graph, 0, &[]);
        assert!(result.is_ok(), "Failed to import: {:?}", result.err());

        let lines = result.expect("Import should succeed");

        // Find a line that contains BR 229-02
        let br_line = lines.iter()
            .find(|line| line.manual_departures.iter()
                .any(|dep| dep.train_number.as_deref() == Some("BR 229-02")))
            .expect("No line found containing BR 229-02");

        // Verify the line ID shows it's going in the return direction
        assert!(br_line.id.starts_with("Dortmund"),
            "Return train line should start with 'Dortmund', got: {}", br_line.id);

        // Get the departure for BR 229-02
        let br_departure = br_line.manual_departures.iter()
            .find(|dep| dep.train_number.as_deref() == Some("BR 229-02"))
            .expect("BR 229-02 departure not found");

        // Get station names
        let from_station = graph.get_station_name(br_departure.from_station)
            .expect("From station not found");
        let to_station = graph.get_station_name(br_departure.to_station)
            .expect("To station not found");

        // For a return train, it should go from the last station to the first
        // BR 229-02 goes from Dortmund (station 48) to Den Haag (station 0)
        assert!(from_station.contains("Dortmund"),
            "BR 229-02 should depart from Dortmund, but departs from {from_station}");
        assert!(to_station.contains("Den Haag") || to_station.contains("Haag"),
            "BR 229-02 should arrive at Den Haag, but arrives at {to_station}");

        // Verify we have route segments
        assert!(!br_line.forward_route.is_empty(), "BR 229-02 line should have route segments");

        // Verify we have both forward and return lines
        let haag_to_dortmund = lines.iter()
            .filter(|line| line.id.starts_with("Den Haag") && line.id.contains("Dortmund"))
            .count();
        let dortmund_to_haag = lines.iter()
            .filter(|line| line.id.starts_with("Dortmund") && line.id.contains("Den Haag"))
            .count();

        assert!(haag_to_dortmund > 0, "Should have at least one forward line");
        assert!(dortmund_to_haag > 0, "Should have at least one return line");
    }

    #[test]
    #[allow(clippy::excessive_nesting)]
    fn test_pattern_grouping_with_platforms() {
        let xml_content = std::fs::read_to_string("dortmund.fpl")
            .expect("Failed to read dortmund.fpl");

        let timetable = parse_jtraingraph(&xml_content)
            .expect("Failed to parse dortmund.fpl");

        // Group trains by pattern
        let mut pattern_groups: HashMap<StopPattern, Vec<&JTrainGraphTrainInfo>> = HashMap::new();

        // Process ti trains (forward direction)
        for train in &timetable.trains.trains_in {
            if let Some(pattern) = get_stop_pattern(train, false) {
                pattern_groups.entry(pattern).or_default().push(train);
            }
        }

        // Process ta trains (return direction, reversed to forward)
        for train in &timetable.trains.trains_away {
            if let Some(pattern) = get_stop_pattern(train, true) {
                pattern_groups.entry(pattern).or_default().push(train);
            }
        }

        // Check if BR 229-02 was parsed (it's a ta train)
        let has_br_229 = timetable.trains.trains_away.iter().any(|t| t.name == "BR 229-02");
        assert!(has_br_229, "BR 229-02 should be parsed from ta trains");

        // Verify pattern grouping includes NS(IC)-4224
        let has_ic_4224 = pattern_groups.values()
            .any(|trains| trains.iter().any(|t| t.name == "NS(IC)-4224"));
        assert!(has_ic_4224, "NS(IC)-4224 should be in pattern groups");
    }
}
