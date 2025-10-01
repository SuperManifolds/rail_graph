use serde::{Deserialize, Serialize};
use super::{Line, RailwayGraph};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub lines: Vec<Line>,
    pub graph: RailwayGraph,
    pub version: u32,
}

impl Project {
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            version: 1,
        }
    }

    pub fn new(lines: Vec<Line>, graph: RailwayGraph) -> Self {
        Self {
            lines,
            graph,
            version: 1,
        }
    }
}
