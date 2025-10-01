use serde::{Deserialize, Serialize};
use super::{Line, RailwayGraph, SegmentState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub lines: Vec<Line>,
    pub graph: RailwayGraph,
    pub segment_state: SegmentState,
    pub version: u32,
}

impl Project {
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            segment_state: SegmentState { double_tracked_segments: std::collections::HashSet::new() },
            version: 1,
        }
    }

    pub fn new(lines: Vec<Line>, graph: RailwayGraph, segment_state: SegmentState) -> Self {
        Self {
            lines,
            graph,
            segment_state,
            version: 1,
        }
    }
}
