mod junction;
mod line;
mod node;
mod project;
mod railway_graph;
mod station;
mod track;

pub use junction::{Junction, RoutingRule};
pub use line::{Line, ScheduleMode, ManualDeparture, RouteSegment};
pub use node::Node;
pub use project::{Project, Legend};
pub use railway_graph::{RailwayGraph, Stations, Tracks, Routes, Junctions, Nodes};
pub use station::{StationNode, Platform};
pub use track::{TrackSegment, Track, TrackDirection};

#[derive(Clone, Copy, PartialEq)]
pub enum RouteDirection {
    Forward,
    Return,
}

