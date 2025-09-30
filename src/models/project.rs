use serde::{Deserialize, Serialize};
use super::{Line, Station, SegmentState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub lines: Vec<Line>,
    pub stations: Vec<Station>,
    pub segment_state: SegmentState,
    pub version: u32,
}

impl Project {
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            stations: Vec::new(),
            segment_state: SegmentState { double_tracked_segments: std::collections::HashSet::new() },
            version: 1,
        }
    }

    pub fn new(lines: Vec<Line>, stations: Vec<Station>, segment_state: SegmentState) -> Self {
        Self {
            lines,
            stations,
            segment_state,
            version: 1,
        }
    }
}
