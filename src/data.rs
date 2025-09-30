use crate::models::{Line, Station};

/// Parse CSV data into lines and stations
pub fn parse_csv_data() -> (Vec<Line>, Vec<Station>) {
    let csv_content = include_str!("../lines.csv");
    parse_csv_string(csv_content)
}

/// Parse CSV string into lines and stations
pub fn parse_csv_string(csv_content: &str) -> (Vec<Line>, Vec<Station>) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_content.as_bytes());

    let mut records = reader.records();

    let Some(Ok(header)) = records.next() else {
        return (Vec::new(), Vec::new());
    };

    let line_ids = extract_line_ids(&header);
    let lines = Line::create_from_ids(&line_ids);
    let stations = Station::parse_from_csv(records, &line_ids);

    (lines, stations)
}

fn extract_line_ids(header: &csv::StringRecord) -> Vec<String> {
    header.iter()
        .skip(1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}