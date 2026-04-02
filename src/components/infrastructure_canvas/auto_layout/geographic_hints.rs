use petgraph::stable_graph::NodeIndex;
use std::collections::HashMap;

/// Geographic hints for layout - provides preferred directions based on real-world coordinates
#[derive(Debug, Clone, Default)]
pub struct GeographicHints {
    /// Map from `NodeIndex` to (longitude, latitude) coordinates
    pub(crate) lonlat_map: HashMap<NodeIndex, (f64, f64)>,
}

impl GeographicHints {
    /// Create empty hints (no geographic data)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            lonlat_map: HashMap::new(),
        }
    }

    /// Create hints from a lonlat map
    #[must_use]
    pub fn new(lonlat_map: HashMap<NodeIndex, (f64, f64)>) -> Self {
        Self { lonlat_map }
    }

    /// Check if hints are available
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lonlat_map.is_empty()
    }

    /// Get number of nodes with geographic hints
    #[must_use]
    pub fn len(&self) -> usize {
        self.lonlat_map.len()
    }

    /// Check if a node has geographic coordinates
    #[must_use]
    pub fn has_coords(&self, node: NodeIndex) -> bool {
        self.lonlat_map.contains_key(&node)
    }

    /// Get coordinates for a node
    #[must_use]
    pub fn get_coords(&self, node: NodeIndex) -> Option<(f64, f64)> {
        self.lonlat_map.get(&node).copied()
    }

    /// Get preferred direction from one node to another based on geography
    /// Returns angle in radians where 0 = East, -π/2 = North (screen up)
    #[must_use]
    pub fn preferred_direction(&self, from: NodeIndex, to: NodeIndex) -> Option<f64> {
        let from_lonlat = self.lonlat_map.get(&from)?;
        let to_lonlat = self.lonlat_map.get(&to)?;

        let dx = to_lonlat.0 - from_lonlat.0; // East is positive
        let dy = from_lonlat.1 - to_lonlat.1; // Invert: North (higher lat) = negative Y (up on screen)

        Some(dy.atan2(dx))
    }
}
