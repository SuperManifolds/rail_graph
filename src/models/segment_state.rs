use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SegmentState {
    // Key is the index of the second station in the segment
    // So segment between stations[i] and stations[i+1] is stored at key i+1
    pub double_tracked_segments: HashSet<usize>,
}