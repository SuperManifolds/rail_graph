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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_direction_equality() {
        assert_eq!(TrackDirection::Bidirectional, TrackDirection::Bidirectional);
        assert_eq!(TrackDirection::Forward, TrackDirection::Forward);
        assert_eq!(TrackDirection::Backward, TrackDirection::Backward);
        assert_ne!(TrackDirection::Forward, TrackDirection::Backward);
    }

    #[test]
    fn test_track_creation() {
        let track = Track { direction: TrackDirection::Bidirectional };
        assert_eq!(track.direction, TrackDirection::Bidirectional);
    }

    #[test]
    fn test_new_single_track() {
        let segment = TrackSegment::new_single_track();
        assert_eq!(segment.tracks.len(), 1);
        assert_eq!(segment.tracks[0].direction, TrackDirection::Bidirectional);
        assert_eq!(segment.distance, None);
    }

    #[test]
    fn test_new_double_track() {
        let segment = TrackSegment::new_double_track();
        assert_eq!(segment.tracks.len(), 2);
        assert_eq!(segment.tracks[0].direction, TrackDirection::Forward);
        assert_eq!(segment.tracks[1].direction, TrackDirection::Backward);
        assert_eq!(segment.distance, None);
    }

    #[test]
    fn test_track_segment_with_distance() {
        let segment = TrackSegment {
            tracks: vec![Track { direction: TrackDirection::Bidirectional }],
            distance: Some(100.5),
        };
        assert_eq!(segment.tracks.len(), 1);
        assert_eq!(segment.distance, Some(100.5));
    }
}
