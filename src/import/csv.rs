use chrono::Duration;
use std::collections::HashMap;
use crate::models::{Line, RailwayGraph, RouteSegment, Stations, Tracks};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    StationName,
    Platform,
    TrackDistance,
    TrackNumber,
    ArrivalTime,
    DepartureTime,
    TravelTime,
    WaitTime,
    Offset,
    Skip,
}

impl ColumnType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StationName => "Station Name",
            Self::Platform => "Platform",
            Self::TrackDistance => "Track Distance",
            Self::TrackNumber => "Track Number",
            Self::ArrivalTime => "Arrival Time",
            Self::DepartureTime => "Departure Time",
            Self::TravelTime => "Travel Time",
            Self::WaitTime => "Wait Time",
            Self::Offset => "Offset",
            Self::Skip => "Skip",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnMapping {
    pub column_index: usize,
    pub column_type: ColumnType,
    pub header: Option<String>,
    pub sample_values: Vec<String>,
    pub auto_detected_type: ColumnType,
    /// Group index for repeating patterns (e.g., columns 1-3 are group 0, columns 4-6 are group 1)
    pub group_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ImportDefaults {
    pub default_wait_time: Duration,
    pub per_line_wait_times: HashMap<String, Duration>,
}

impl Default for ImportDefaults {
    fn default() -> Self {
        Self {
            default_wait_time: Duration::seconds(30),
            per_line_wait_times: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CsvImportConfig {
    pub columns: Vec<ColumnMapping>,
    pub has_headers: bool,
    pub defaults: ImportDefaults,
    /// If Some(n), columns repeat every n columns (for multi-line grouped format)
    pub pattern_repeat: Option<usize>,
    /// Line names for each group (when `pattern_repeat` is set)
    pub group_line_names: HashMap<usize, String>,
}

/// Analyze CSV content and suggest column mappings
pub fn analyze_csv(content: &str) -> Option<CsvImportConfig> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(content.as_bytes());

    let mut records = reader.records();

    // Read first row (potential header)
    let first_row = records.next()?.ok()?;

    // Read up to 5 more rows for samples
    let mut sample_rows = Vec::new();
    for _ in 0..5 {
        if let Some(Ok(row)) = records.next() {
            sample_rows.push(row);
        } else {
            break;
        }
    }

    // Determine if first row is a header
    let has_headers = looks_like_header(&first_row);

    // Build column mappings
    let mut columns = Vec::new();
    for (col_idx, field) in first_row.iter().enumerate() {
        let header = if has_headers {
            Some(field.to_string())
        } else {
            None
        };

        // Collect sample values
        let sample_values: Vec<String> = if has_headers {
            // Use the data rows
            sample_rows.iter()
                .filter_map(|row| row.get(col_idx))
                .map(ToString::to_string)
                .collect()
        } else {
            // Use first row + data rows
            std::iter::once(field)
                .chain(sample_rows.iter().filter_map(|row| row.get(col_idx)))
                .map(ToString::to_string)
                .collect()
        };

        let detected_type = detect_column_type(header.as_deref(), &sample_values, &columns);

        columns.push(ColumnMapping {
            column_index: col_idx,
            column_type: detected_type,
            header,
            sample_values: sample_values.into_iter().take(5).collect(),
            auto_detected_type: detected_type,
            group_index: None,
        });
    }

    // Detect repeating patterns
    let (pattern_repeat, group_assignments, group_line_names) = detect_column_grouping(&columns);

    // Apply group assignments if pattern detected
    if pattern_repeat.is_some() {
        for col in &mut columns {
            col.group_index = group_assignments.get(&col.column_index).copied();
        }
    }

    Some(CsvImportConfig {
        columns,
        has_headers,
        defaults: ImportDefaults::default(),
        pattern_repeat,
        group_line_names,
    })
}

/// Detect repeating column patterns for multi-line grouped format
/// Returns (`pattern_repeat`, `group_assignments`, `group_line_names`) where:
/// - `pattern_repeat`: Some(n) if columns repeat every n columns
/// - `group_assignments`: Map of `column_index` -> `group_index`
/// - `group_line_names`: Map of `group_index` -> line name extracted from headers
fn detect_column_grouping(columns: &[ColumnMapping]) -> (Option<usize>, HashMap<usize, usize>, HashMap<usize, String>) {
    let mut group_assignments = HashMap::new();
    let mut group_line_names = HashMap::new();

    // Find the station column (should be first)
    let station_col_idx = columns.iter()
        .position(|c| c.column_type == ColumnType::StationName);

    let Some(station_idx) = station_col_idx else {
        return (None, group_assignments, group_line_names);
    };

    // Get columns after the station column (excluding global columns like distance)
    let data_columns: Vec<&ColumnMapping> = columns.iter()
        .filter(|c| {
            c.column_index > station_idx
            && c.column_type != ColumnType::Skip
            && c.column_type != ColumnType::TrackDistance
        })
        .collect();

    if data_columns.len() < 2 {
        return (None, group_assignments, group_line_names);
    }

    // Try different pattern lengths (from 2 to half the number of data columns)
    // Note: We start at 2 because pattern_len=1 means each column is a separate line (simple format)
    let max_pattern_len = data_columns.len() / 2;

    // Special case: If all columns are the same type and there are multiple columns,
    // this is likely the simple format (one line per column), not a grouped format
    let all_same_type = data_columns.windows(2).all(|w| w[0].column_type == w[1].column_type);
    if all_same_type && data_columns.len() > 1 {
        // This is the simple format - each column is a separate line
        return (None, group_assignments, group_line_names);
    }

    for pattern_len in 2..=max_pattern_len {
        if data_columns.len() % pattern_len != 0 {
            continue;
        }

        let num_groups = data_columns.len() / pattern_len;
        let mut pattern_matches = true;

        // Check if the pattern repeats
        for group_idx in 1..num_groups {
            for pos in 0..pattern_len {
                let first_col = data_columns[pos];
                let curr_col = data_columns[group_idx * pattern_len + pos];

                // Check if column types match
                if first_col.column_type != curr_col.column_type {
                    pattern_matches = false;
                    break;
                }
            }
            if !pattern_matches {
                break;
            }
        }

        // If pattern matches, assign groups and extract line names
        if pattern_matches {
            for (idx, col) in data_columns.iter().enumerate() {
                let group_idx = idx / pattern_len;
                group_assignments.insert(col.column_index, group_idx);
            }

            // Try to extract line names from headers
            let num_groups = data_columns.len() / pattern_len;
            for group_idx in 0..num_groups {
                // Look at all headers in this group
                let group_headers: Vec<Option<&str>> = (0..pattern_len)
                    .map(|pos| {
                        let col_idx = group_idx * pattern_len + pos;
                        data_columns.get(col_idx).and_then(|c| c.header.as_deref())
                    })
                    .collect();

                // Try to extract a common identifier from headers
                if let Some(line_name) = extract_line_identifier(&group_headers) {
                    group_line_names.insert(group_idx, line_name);
                }
            }

            return (Some(pattern_len), group_assignments, group_line_names);
        }
    }

    (None, group_assignments, group_line_names)
}

/// Extract a line identifier from column headers
/// Looks for common patterns like "Line1", "L1", "R70", etc.
#[must_use]
pub fn extract_line_identifier(headers: &[Option<&str>]) -> Option<String> {
    // Find headers that contain alphanumeric characters
    let valid_headers: Vec<&str> = headers.iter()
        .filter_map(|h| *h)
        .filter(|h| !h.trim().is_empty())
        .collect();

    if valid_headers.is_empty() {
        return None;
    }

    // Try to find a common prefix or identifier
    // Pattern 1: Look for numbers/letters at the start (e.g., "R70 Time" -> "R70", "Line1 Arr" -> "Line1")
    for header in &valid_headers {
        // Extract leading alphanumeric sequence
        let identifier: String = header.chars()
            .take_while(|c| c.is_alphanumeric())
            .collect();

        if !identifier.is_empty() && identifier.chars().any(char::is_numeric) {
            return Some(identifier);
        }
    }

    // Pattern 2: Look for a number anywhere in the header
    for header in &valid_headers {
        if let Some(num_start) = header.find(|c: char| c.is_numeric()) {
            // Extract the number and any adjacent letters
            let before_num: String = header[..num_start].chars()
                .rev()
                .take_while(|c| c.is_alphabetic())
                .collect::<String>()
                .chars()
                .rev()
                .collect();

            let num_and_after: String = header[num_start..].chars()
                .take_while(|c| c.is_alphanumeric())
                .collect();

            let identifier = format!("{before_num}{num_and_after}");
            if !identifier.is_empty() {
                return Some(identifier);
            }
        }
    }

    None
}

/// Check if a row looks like a header
fn looks_like_header(row: &csv::StringRecord) -> bool {
    // Header heuristic: if first field contains "station" or "stop", likely a header
    if let Some(first) = row.get(0) {
        let lower = first.to_lowercase();
        if lower.contains("station") || lower.contains("stop") || lower.contains("name") {
            return true;
        }
    }

    // If any field looks like time data, probably not a header
    for field in row {
        if is_time_format(field) || is_duration_format(field) {
            return false;
        }
    }

    // Check if fields look like labels (contain letters, not just numbers/times)
    let text_fields = row.iter()
        .filter(|f| !f.trim().is_empty())
        .filter(|f| f.chars().any(char::is_alphabetic))
        .count();

    text_fields >= row.len() / 2
}

/// Detect the type of a column based on header, sample values, and previously processed columns
fn detect_column_type(header: Option<&str>, samples: &[String], prev_columns: &[ColumnMapping]) -> ColumnType {
    let non_empty_samples: Vec<&str> = samples.iter()
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
        .collect();

    if non_empty_samples.is_empty() {
        return ColumnType::Skip;
    }

    // Check if there's already a station column
    let has_station = prev_columns.iter().any(|c| c.column_type == ColumnType::StationName);

    // Get the previous column type (for context-aware detection)
    let prev_col_type = prev_columns.last().map(|c| c.column_type);

    // Check header-based detection first (these take precedence)
    if let Some(h) = header {
        let lower = h.to_lowercase();

        if (lower.contains("station") || lower.contains("stop")) && !has_station {
            return ColumnType::StationName;
        }
        if lower.contains("offset") {
            return ColumnType::Offset;
        }
        if lower.contains("platform") || lower.contains("plat") {
            return ColumnType::Platform;
        }
        if lower.contains("distance") || lower.contains("km") {
            return ColumnType::TrackDistance;
        }
        if lower.contains("track") {
            return ColumnType::TrackNumber;
        }
        if lower.contains("arr") {
            return ColumnType::ArrivalTime;
        }
        if lower.contains("dep") {
            return ColumnType::DepartureTime;
        }
        if lower.contains("travel") || lower.contains("duration") {
            return ColumnType::TravelTime;
        }
        if lower.contains("wait") || lower.contains("dwell") {
            return ColumnType::WaitTime;
        }
    }

    // Data-based detection

    // Station: no existing station and values don't contain numbers
    if !has_station {
        let has_numbers = non_empty_samples.iter().any(|s| s.chars().any(char::is_numeric));
        if !has_numbers {
            return ColumnType::StationName;
        }
    }

    // Check if all samples are numeric
    let all_integer = non_empty_samples.iter().all(|s| s.trim().parse::<i32>().is_ok());
    let all_numeric = non_empty_samples.iter().all(|s| s.trim().parse::<f64>().is_ok());

    if all_numeric {
        // Platform: integer higher than 1
        if all_integer {
            let values: Vec<i32> = non_empty_samples.iter()
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();
            if values.iter().any(|&v| v > 1) {
                return ColumnType::Platform;
            }
            // Track number: integers of 1 and 2
            if values.iter().all(|&v| v == 1 || v == 2) {
                return ColumnType::TrackNumber;
            }
        }

        // Track distance: contains floats
        let has_decimals = non_empty_samples.iter().any(|s| s.contains('.'));
        if has_decimals {
            return ColumnType::TrackDistance;
        }
    }

    // Time-based detection
    let all_times = non_empty_samples.iter().all(|s| is_time_format(s));
    if all_times {
        // Check if all times are very short (< 1 hour) - likely durations, not times of day
        let durations: Vec<Duration> = non_empty_samples.iter()
            .filter_map(|s| parse_time_to_duration(s))
            .collect();

        if !durations.is_empty() {
            let all_short = durations.iter().all(|d| d.num_hours() < 1);

            if all_short {
                // These are durations, not times of day
                #[allow(clippy::cast_precision_loss)]
                let avg_seconds = durations.iter().map(chrono::TimeDelta::num_seconds).sum::<i64>() as f64
                    / durations.len() as f64;

                // Wait time: 2 minutes or lower
                if avg_seconds <= 120.0 {
                    return ColumnType::WaitTime;
                }
                // Travel time: larger than 2 minutes but less than 1 hour
                return ColumnType::TravelTime;
            }
        }

        // Offset: starts at 00:00:00 and increases
        if let Some(first) = non_empty_samples.first().and_then(|s| parse_time_to_duration(s)) {
            if first.num_seconds() == 0 && is_monotonically_increasing(&non_empty_samples) {
                return ColumnType::Offset;
            }
        }

        // Check if times are past 4 hours
        let has_time_past_4h = non_empty_samples.iter().any(|s| {
            if let Some(duration) = parse_time_to_duration(s) {
                duration.num_hours() >= 4
            } else {
                false
            }
        });

        if has_time_past_4h {
            // Arrival if no arrival to the left, Departure if there is
            if prev_col_type == Some(ColumnType::ArrivalTime) {
                return ColumnType::DepartureTime;
            }
            return ColumnType::ArrivalTime;
        }

        // Default: times less than 4 hours that aren't offset -> treat as Arrival/Departure based on context
        if prev_col_type == Some(ColumnType::ArrivalTime) {
            return ColumnType::DepartureTime;
        }
        return ColumnType::ArrivalTime;
    }

    // Duration-based detection
    let all_durations = non_empty_samples.iter().all(|s| is_duration_format(s));
    if all_durations {
        // Calculate average duration
        let durations: Vec<Duration> = non_empty_samples.iter()
            .filter_map(|s| parse_time_to_duration(s))
            .collect();

        if !durations.is_empty() {
            #[allow(clippy::cast_precision_loss)]
            let avg_seconds = durations.iter().map(chrono::TimeDelta::num_seconds).sum::<i64>() as f64
                / durations.len() as f64;

            // Wait time: 2 minutes or lower
            if avg_seconds <= 120.0 {
                return ColumnType::WaitTime;
            }
            // Travel time: larger than 2 minutes
            return ColumnType::TravelTime;
        }
    }

    // Default to skip if uncertain
    ColumnType::Skip
}

/// Ensure an edge has enough tracks for the given track number (0-indexed)
/// If `track_number` is Some(N), ensures at least N+1 tracks exist
fn ensure_track_count(graph: &mut RailwayGraph, edge_idx: EdgeIndex, track_number: Option<usize>) {
    let Some(track_num) = track_number else { return };
    let required_track_count = track_num + 1; // Convert 0-indexed to count

    let Some(track_segment) = graph.graph.edge_weight_mut(edge_idx) else { return };
    if track_segment.tracks.len() < required_track_count {
        // Need to add more tracks - recreate with the new count
        track_segment.tracks = super::shared::create_tracks_with_count(required_track_count);
    }
}

/// Normalize time with midnight wraparound detection
/// If current time < previous time, adds 24 hours to current time
fn normalize_time_with_wraparound(time: Duration, prev_time: Option<Duration>) -> Duration {
    if let Some(prev) = prev_time {
        if time < prev {
            return time + Duration::hours(24);
        }
    }
    time
}

/// Check if time values are monotonically increasing (for offset detection)
fn is_monotonically_increasing(samples: &[&str]) -> bool {
    let Some(first) = samples.first().and_then(|s| parse_time_to_duration(s)) else { return false };
    let mut prev_dur = first;

    for sample in samples.iter().skip(1) {
        let Some(curr) = parse_time_to_duration(sample) else { continue };
        if curr < prev_dur {
            return false;
        }
        prev_dur = curr;
    }
    true
}

/// Check if a string looks like a time format (H:MM:SS, HH:MM, etc.)
fn is_time_format(s: &str) -> bool {
    let s = s.trim();

    // Check for H:MM:SS or HH:MM:SS format
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return false;
    }

    // All parts should be numeric
    parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// Check if a string looks like a duration format
fn is_duration_format(s: &str) -> bool {
    let s = s.trim().to_lowercase();

    // Check for patterns like "5min", "30s", "1h", etc.
    if s.chars().next().is_some_and(char::is_numeric)
        && (s.contains("min") || s.contains('s') || s.contains('h')) {
        return true;
    }

    // Also accept plain time format as duration
    is_time_format(&s)
}

/// Parse CSV content with the given column mapping configuration
#[must_use]
pub fn parse_csv_with_mapping(content: &str, config: &CsvImportConfig) -> (Vec<Line>, RailwayGraph) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(content.as_bytes());

    let mut records = reader.records();

    // Skip header row if present
    if config.has_headers {
        let _ = records.next();
    }

    let line_groups = build_line_groups(config);
    if line_groups.is_empty() {
        return (Vec::new(), RailwayGraph::new());
    }

    let line_ids: Vec<String> = line_groups.iter().map(|g| g.line_name.clone()).collect();
    let mut lines = Line::create_from_ids(&line_ids);
    let mut graph = RailwayGraph::new();

    let station_data = collect_station_data(&mut records, config, &line_groups);

    build_routes(&mut lines, &mut graph, &station_data, &line_groups, config);

    (lines, graph)
}

/// Build line groups from column configuration
fn build_line_groups(config: &CsvImportConfig) -> Vec<LineGroupData> {
    if config.pattern_repeat.is_some() {
        // Grouped format: columns repeat every pattern_len columns
        let num_groups = config.columns.iter()
            .filter_map(|c| c.group_index)
            .max()
            .map_or(0, |max_idx| max_idx + 1);

        (0..num_groups).map(|group_idx| {
            let group_columns: Vec<&ColumnMapping> = config.columns.iter()
                .filter(|c| c.group_index == Some(group_idx))
                .collect();

            let arrival_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::ArrivalTime)
                .map(|c| c.column_index);

            let departure_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::DepartureTime)
                .map(|c| c.column_index);

            let offset_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::Offset)
                .map(|c| c.column_index);

            let travel_time_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::TravelTime)
                .map(|c| c.column_index);

            let wait_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::WaitTime)
                .map(|c| c.column_index);

            let platform_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::Platform)
                .map(|c| c.column_index);

            let track_num_col = group_columns.iter()
                .find(|c| c.column_type == ColumnType::TrackNumber)
                .map(|c| c.column_index);

            // Track distance is a global column, not per-group
            let track_dist_col = config.columns.iter()
                .find(|c| c.column_type == ColumnType::TrackDistance && c.group_index.is_none())
                .map(|c| c.column_index);

            let line_name = config.group_line_names.get(&group_idx)
                .cloned()
                .unwrap_or_else(|| format!("Line {}", group_idx + 1));

            LineGroupData {
                line_name,
                arrival_time_column: arrival_col,
                departure_time_column: departure_col,
                offset_column: offset_col,
                travel_time_column: travel_time_col,
                wait_column: wait_col,
                platform_column: platform_col,
                track_number_column: track_num_col,
                track_distance_column: track_dist_col,
            }
        }).filter(|g| g.arrival_time_column.is_some() || g.departure_time_column.is_some() || g.offset_column.is_some() || g.travel_time_column.is_some())
          .collect()
    } else {
        // Simple format: each time/travel-time column is a separate line
        let time_columns: Vec<_> = config.columns.iter()
            .filter(|c| matches!(c.column_type, ColumnType::ArrivalTime | ColumnType::DepartureTime | ColumnType::Offset))
            .collect();

        let travel_time_columns: Vec<_> = config.columns.iter()
            .filter(|c| c.column_type == ColumnType::TravelTime)
            .collect();

        // Combine time and travel time columns
        let all_time_columns = time_columns.into_iter().chain(travel_time_columns);

        all_time_columns.map(|c| {
            let line_name = c.header.clone()
                .unwrap_or_else(|| format!("Line {}", c.column_index));

            // Determine which column type this is
            let (arrival_col, departure_col, offset_col, travel_col) = match c.column_type {
                ColumnType::ArrivalTime => (Some(c.column_index), None, None, None),
                ColumnType::DepartureTime => (None, Some(c.column_index), None, None),
                ColumnType::Offset => (None, None, Some(c.column_index), None),
                ColumnType::TravelTime => (None, None, None, Some(c.column_index)),
                _ => (None, None, None, None),
            };

            // In simple format, look for global wait/platform/track columns
            let wait_col = config.columns.iter()
                .find(|col| col.column_type == ColumnType::WaitTime && col.group_index.is_none())
                .map(|col| col.column_index);

            let platform_col = config.columns.iter()
                .find(|col| col.column_type == ColumnType::Platform && col.group_index.is_none())
                .map(|col| col.column_index);

            let track_num_col = config.columns.iter()
                .find(|col| col.column_type == ColumnType::TrackNumber && col.group_index.is_none())
                .map(|col| col.column_index);

            let track_dist_col = config.columns.iter()
                .find(|col| col.column_type == ColumnType::TrackDistance && col.group_index.is_none())
                .map(|col| col.column_index);

            LineGroupData {
                line_name,
                arrival_time_column: arrival_col,
                departure_time_column: departure_col,
                offset_column: offset_col,
                travel_time_column: travel_col,
                wait_column: wait_col,
                platform_column: platform_col,
                track_number_column: track_num_col,
                track_distance_column: track_dist_col,
            }
        })
        .collect()
    }
}

/// Collect station data from CSV records
fn collect_station_data(
    records: &mut dyn Iterator<Item = Result<csv::StringRecord, csv::Error>>,
    config: &CsvImportConfig,
    line_groups: &[LineGroupData],
) -> Vec<StationRowData> {
    let mut station_data: Vec<StationRowData> = Vec::new();

    for record in records {
        let Ok(row) = record else { continue };

        // Extract station name
        let station_name = config.columns.iter()
            .find(|c| c.column_type == ColumnType::StationName)
            .and_then(|c| row.get(c.column_index))
            .filter(|s| !s.trim().is_empty());

        let Some(station_name) = station_name else { continue };

        // Extract data for each line group
        let mut line_data = Vec::new();
        for group in line_groups {
            let arrival_time = group.arrival_time_column
                .and_then(|col| row.get(col))
                .filter(|s| !s.trim().is_empty())
                .and_then(parse_time_to_duration)
                .or_else(|| {
                    // Fallback to offset column as arrival time
                    group.offset_column
                        .and_then(|col| row.get(col))
                        .filter(|s| !s.trim().is_empty())
                        .and_then(parse_time_to_duration)
                });

            let departure_time = group.departure_time_column
                .and_then(|col| row.get(col))
                .filter(|s| !s.trim().is_empty())
                .and_then(parse_time_to_duration);

            let travel_time = group.travel_time_column
                .and_then(|col| row.get(col))
                .filter(|s| !s.trim().is_empty())
                .and_then(parse_time_to_duration);

            let wait_time = group.wait_column
                .and_then(|col| row.get(col))
                .filter(|s| !s.trim().is_empty())
                .and_then(parse_time_to_duration);

            let platform = group.platform_column
                .and_then(|col| row.get(col))
                .filter(|s| !s.trim().is_empty())
                .map(std::string::ToString::to_string);

            let track_distance = group.track_distance_column
                .and_then(|col| row.get(col))
                .and_then(|s| s.trim().parse::<f64>().ok());

            let track_number = group.track_number_column
                .and_then(|col| row.get(col))
                .and_then(|s| s.trim().parse::<usize>().ok());

            line_data.push(LineStationData {
                arrival_time,
                departure_time,
                travel_time,
                wait_time,
                platform,
                track_distance,
                track_number,
            });
        }

        station_data.push(StationRowData {
            name: station_name.to_string(),
            line_data,
        });
    }

    station_data
}

/// Build routes and graph from station data
fn build_routes(
    lines: &mut [Line],
    graph: &mut RailwayGraph,
    station_data: &[StationRowData],
    line_groups: &[LineGroupData],
    config: &CsvImportConfig,
) {
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), EdgeIndex> = HashMap::new();

    for (line_idx, group) in line_groups.iter().enumerate() {
        let mut route = Vec::new();
        let mut prev_station: Option<(NodeIndex, Duration)> = None;

        // Track cumulative time for TravelTime format
        let mut cumulative_time_tracker = Duration::zero();
        let uses_travel_time = group.travel_time_column.is_some()
            && group.arrival_time_column.is_none()
            && group.departure_time_column.is_none()
            && group.offset_column.is_none();

        // Get wait time for this line (fallback chain: per-line -> default)
        let default_wait_time = config.defaults.per_line_wait_times
            .get(&group.line_name)
            .copied()
            .unwrap_or(config.defaults.default_wait_time);

        for station in station_data {
            let line_station_data = &station.line_data[line_idx];

            // Determine cumulative time: either from time column or accumulated from travel times
            let cumulative_time = if uses_travel_time {
                // For travel time format, accumulate durations
                // First station starts at 0:00:00, subsequent stations add travel time
                if prev_station.is_none() {
                    cumulative_time_tracker = Duration::zero();
                    Some(cumulative_time_tracker)
                } else if let Some(travel) = line_station_data.travel_time {
                    cumulative_time_tracker += travel;
                    Some(cumulative_time_tracker)
                } else {
                    None
                }
            } else {
                // Handle arrival time with midnight wraparound detection
                line_station_data.arrival_time.map(|time| {
                    normalize_time_with_wraparound(time, prev_station.map(|(_, t)| t))
                })
            };

            let Some(cumulative_time) = cumulative_time else {
                continue;
            };

            // Check for passing loop marker
            let is_passing_loop = station.name.ends_with("(P)");

            let clean_name = station.name
                .trim_end_matches("(P)")
                .trim_end_matches("(J)")
                .trim()
                .to_string();

            // Get or create station node
            let station_idx = graph.add_or_get_station(clean_name);

            // Mark as passing loop if needed
            if is_passing_loop {
                if let Some(station_node) = graph.graph.node_weight_mut(station_idx)
                    .and_then(|node| node.as_station_mut()) {
                    station_node.passing_loop = true;
                }
            }

            // If there was a previous station, create or reuse edge
            let Some((prev_idx, prev_time)) = prev_station else {
                prev_station = Some((station_idx, cumulative_time));
                continue;
            };

            let travel_time = cumulative_time - prev_time;

            // Check if edge already exists, or create new track
            let edge_idx = *edge_map.entry((prev_idx, station_idx))
                .or_insert_with(|| {
                    // Default to 1 bidirectional track if no track info
                    let tracks = super::shared::create_tracks_with_count(1);
                    graph.add_track(prev_idx, station_idx, tracks)
                });

            // Ensure edge has enough tracks for the requested track index
            ensure_track_count(graph, edge_idx, line_station_data.track_number);

            // Determine wait time based on priority:
            // 1. Passing loops always have 0 wait time
            // 2. If both arrival and departure times are present, calculate from difference
            // 3. Use wait_time column if present
            // 4. Fall back to default wait time
            let station_wait_time = if is_passing_loop {
                Duration::seconds(0)
            } else if let (Some(arrival), Some(departure)) = (line_station_data.arrival_time, line_station_data.departure_time) {
                // Handle midnight wraparound for departure time
                let normalized_departure = normalize_time_with_wraparound(departure, Some(arrival));
                normalized_departure - arrival
            } else {
                line_station_data.wait_time.unwrap_or(default_wait_time)
            };

            // Handle platform assignment
            let (origin_platform, destination_platform) = if let Some(ref platform_name) = line_station_data.platform {
                // Add platform to destination station if not exists and get its index
                let dest_platform_idx = super::shared::get_or_add_platform(graph, station_idx, platform_name);

                // For origin, use default platform selection
                let origin_platforms = graph.graph.node_weight(prev_idx)
                    .and_then(|n| n.as_station())
                    .map_or(1, |s| s.platforms.len());
                let origin_platform_idx = graph.get_default_platform_for_arrival(edge_idx, false, origin_platforms);

                (origin_platform_idx, dest_platform_idx)
            } else {
                // No platform specified, use defaults
                let origin_platforms = graph.graph.node_weight(prev_idx)
                    .and_then(|n| n.as_station())
                    .map_or(1, |s| s.platforms.len());

                let dest_platforms = graph.graph.node_weight(station_idx)
                    .and_then(|n| n.as_station())
                    .map_or(1, |s| s.platforms.len());

                let origin_platform_idx = graph.get_default_platform_for_arrival(edge_idx, false, origin_platforms);
                let dest_platform_idx = graph.get_default_platform_for_arrival(edge_idx, true, dest_platforms);

                (origin_platform_idx, dest_platform_idx)
            };

            // Use track number from CSV if provided, otherwise default to 0
            let track_index = line_station_data.track_number.unwrap_or(0);

            route.push(RouteSegment {
                edge_index: edge_idx.index(),
                track_index,
                origin_platform,
                destination_platform,
                duration: Some(travel_time),
                wait_time: station_wait_time,
            });

            prev_station = Some((station_idx, cumulative_time));
        }

        // Assign forward route
        lines[line_idx].forward_route.clone_from(&route);

        // Generate return route
        let mut return_route = Vec::new();
        for i in (0..route.len()).rev() {
            let forward_segment = &route[i];
            let edge_idx = petgraph::graph::EdgeIndex::new(forward_segment.edge_index);

            let return_track_index = if let Some(track_segment) = graph.get_track(edge_idx) {
                usize::from(track_segment.tracks.len() > 1)
            } else {
                0
            };

            return_route.push(RouteSegment {
                edge_index: forward_segment.edge_index,
                track_index: return_track_index,
                origin_platform: forward_segment.destination_platform,
                destination_platform: forward_segment.origin_platform,
                duration: forward_segment.duration,
                wait_time: forward_segment.wait_time,
            });
        }
        lines[line_idx].return_route = return_route;
    }
}

struct LineGroupData {
    line_name: String,
    arrival_time_column: Option<usize>,
    departure_time_column: Option<usize>,
    offset_column: Option<usize>,
    travel_time_column: Option<usize>,
    wait_column: Option<usize>,
    platform_column: Option<usize>,
    track_number_column: Option<usize>,
    track_distance_column: Option<usize>,
}

struct StationRowData {
    name: String,
    line_data: Vec<LineStationData>,
}

struct LineStationData {
    arrival_time: Option<Duration>,
    departure_time: Option<Duration>,
    travel_time: Option<Duration>,
    wait_time: Option<Duration>,
    platform: Option<String>,
    #[allow(dead_code)] // Reserved for future distance-based time calculation
    track_distance: Option<f64>,
    track_number: Option<usize>,
}


/// Parse a time string to Duration
fn parse_time_to_duration(s: &str) -> Option<Duration> {
    use chrono::Timelike;

    crate::time::parse_time_hms(s)
        .ok()
        .map(|t| {
            Duration::hours(i64::from(t.hour())) +
            Duration::minutes(i64::from(t.minute())) +
            Duration::seconds(i64::from(t.second()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_time_format() {
        assert!(is_time_format("0:00:00"));
        assert!(is_time_format("10:30:15"));
        assert!(is_time_format("1:05"));
        assert!(is_time_format("23:59"));

        assert!(!is_time_format("abc"));
        assert!(!is_time_format("10"));
        assert!(!is_time_format(""));
    }

    #[test]
    fn test_is_duration_format() {
        assert!(is_duration_format("5min"));
        assert!(is_duration_format("30s"));
        assert!(is_duration_format("1h"));
        assert!(is_duration_format("0:10:00"));

        assert!(!is_duration_format("abc"));
        assert!(!is_duration_format(""));
    }

    #[test]
    fn test_detect_column_type_with_header() {
        assert_eq!(
            detect_column_type(Some("Station"), &[], &[]),
            ColumnType::StationName
        );
        assert_eq!(
            detect_column_type(Some("Arrival Time"), &[], &[]),
            ColumnType::ArrivalTime
        );
    }

    #[test]
    fn test_detect_column_type_from_samples() {
        let time_samples = vec!["5:00:00".to_string(), "6:10:00".to_string()];
        assert_eq!(
            detect_column_type(None, &time_samples, &[]),
            ColumnType::ArrivalTime
        );

        let duration_samples = vec!["5min".to_string(), "10min".to_string()];
        assert_eq!(
            detect_column_type(None, &duration_samples, &[]),
            ColumnType::TravelTime
        );
    }

    #[test]
    fn test_looks_like_header() {
        let mut header_row = csv::StringRecord::new();
        header_row.push_field("Station");
        header_row.push_field("Line 1");
        header_row.push_field("Line 2");
        assert!(looks_like_header(&header_row));

        let mut data_row = csv::StringRecord::new();
        data_row.push_field("Station A");
        data_row.push_field("0:00:00");
        data_row.push_field("0:00:00");
        assert!(!looks_like_header(&data_row));
    }

    #[test]
    fn test_analyze_csv_simple() {
        let csv = "Station,Line1,Line2\nA,0:00:00,0:00:00\nB,0:10:00,0:15:00\n";
        let config = analyze_csv(csv).expect("Should parse CSV");

        assert!(config.has_headers);
        assert_eq!(config.columns.len(), 3);
        assert_eq!(config.columns[0].column_type, ColumnType::StationName);
        assert_eq!(config.columns[1].column_type, ColumnType::ArrivalTime);
        assert_eq!(config.columns[2].column_type, ColumnType::ArrivalTime);
    }

    #[test]
    fn test_analyze_csv_no_headers() {
        let csv = "Station A,0:00:00,0:00:00\nStation B,0:10:00,0:15:00\n";
        let config = analyze_csv(csv).expect("Should parse CSV");

        assert!(!config.has_headers);
        assert_eq!(config.columns.len(), 3);
        assert_eq!(config.columns[0].column_type, ColumnType::StationName);
    }
}
