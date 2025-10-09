use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TrackDirection {
    Bidirectional,
    Forward,    // From source to target only
    Backward,   // From target to source only
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub direction: TrackDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSegment {
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
}

impl TrackSegment {
    #[must_use]
    pub fn new_single_track() -> Self {
        Self {
            tracks: vec![Track { direction: TrackDirection::Bidirectional }],
            distance: None,
        }
    }

    #[must_use]
    pub fn new_double_track() -> Self {
        Self {
            tracks: vec![
                Track { direction: TrackDirection::Forward },
                Track { direction: TrackDirection::Backward },
            ],
            distance: None,
        }
    }
}
