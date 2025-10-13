use serde::{Deserialize, Serialize};
use super::{Line, RailwayGraph, GraphView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Legend {
    pub show_station_crossings: bool,
    pub show_conflicts: bool,
    pub show_line_blocks: bool,
}

impl Default for Legend {
    fn default() -> Self {
        Self {
            show_station_crossings: true,
            show_conflicts: true,
            show_line_blocks: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub lines: Vec<Line>,
    pub graph: RailwayGraph,
    #[serde(default)]
    pub legend: Legend,
    #[serde(default)]
    pub views: Vec<GraphView>,
    #[serde(default)]
    pub active_tab_id: Option<String>,
}

impl Project {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            legend: Legend::default(),
            views: Vec::new(),
            active_tab_id: None,
        }
    }

    #[must_use]
    pub fn new(lines: Vec<Line>, graph: RailwayGraph, legend: Legend) -> Self {
        Self {
            lines,
            graph,
            legend,
            views: Vec::new(),
            active_tab_id: None,
        }
    }
}
