mod days_of_week;
mod junction;
mod line;
mod node;
mod project;
mod railway_graph;
mod station;
mod track;
mod view;

pub use days_of_week::DaysOfWeek;
pub use junction::{Junction, RoutingRule};
pub use line::{Line, ScheduleMode, ManualDeparture, RouteSegment, generate_random_color};
pub use node::Node;
pub use project::{Project, ProjectMetadata, Legend};
pub use railway_graph::{RailwayGraph, Stations, Tracks, Routes, Junctions};
pub use station::{StationNode, Platform};
pub use track::{TrackSegment, Track, TrackDirection};
pub use view::{GraphView, ViewportState};

#[derive(Clone, Copy, PartialEq)]
pub enum RouteDirection {
    Forward,
    Return,
}

