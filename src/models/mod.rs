mod line;
mod project;
mod railway_graph;
mod station;
mod track;

pub use line::{Line, ScheduleMode, ManualDeparture, RouteSegment};
pub use project::{Project, Legend};
pub use railway_graph::RailwayGraph;
pub use station::{StationNode, Platform};
pub use track::{TrackSegment, Track, TrackDirection};

#[derive(Clone, Copy, PartialEq)]
pub enum RouteDirection {
    Forward,
    Return,
}

