mod days_of_week;
mod junction;
mod keyboard_shortcuts;
mod line;
mod node;
mod project;
mod railway_graph;
mod station;
mod track;
mod user_settings;
mod view;

pub use days_of_week::DaysOfWeek;
pub use junction::{Junction, RoutingRule};
pub use keyboard_shortcuts::{KeyboardShortcut, KeyboardShortcuts, ShortcutCategory, ShortcutMetadata};
pub use line::{Line, ScheduleMode, ManualDeparture, RouteSegment, generate_random_color};
pub use node::Node;
pub use project::{Project, ProjectMetadata, Legend, SpacingMode, ProjectSettings, TrackHandedness};
pub use railway_graph::{RailwayGraph, Stations, Tracks, Routes, Junctions};
pub use station::{StationNode, Platform};
pub use track::{TrackSegment, Track, TrackDirection};
pub use user_settings::UserSettings;
pub use view::{GraphView, ViewportState};

#[derive(Clone, Copy, PartialEq)]
pub enum RouteDirection {
    Forward,
    Return,
}

