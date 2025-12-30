use serde::{Deserialize, Serialize};
use crate::components::infrastructure_canvas::station_renderer::LabelPosition;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub name: String,
}

pub fn default_platforms() -> Vec<Platform> {
    vec![
        Platform { name: "1".to_string() },
        Platform { name: "2".to_string() },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationNode {
    pub name: String,
    #[serde(default)]
    pub position: Option<(f64, f64)>,
    #[serde(default)]
    pub passing_loop: bool,
    #[serde(default = "default_platforms")]
    pub platforms: Vec<Platform>,
    #[serde(default)]
    pub label_position: Option<LabelPosition>,
    /// External ID for imported stations (e.g., NIMBY Rails hex ID)
    #[serde(default)]
    pub external_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_platforms() {
        let platforms = default_platforms();
        assert_eq!(platforms.len(), 2);
        assert_eq!(platforms[0].name, "1");
        assert_eq!(platforms[1].name, "2");
    }

    #[test]
    fn test_station_node_creation() {
        let station = StationNode {
            name: "Test Station".to_string(),
            position: Some((10.0, 20.0)),
            passing_loop: true,
            platforms: vec![Platform { name: "A".to_string() }],
            label_position: None,
            external_id: None,
        };

        assert_eq!(station.name, "Test Station");
        assert_eq!(station.position, Some((10.0, 20.0)));
        assert!(station.passing_loop);
        assert_eq!(station.platforms.len(), 1);
        assert_eq!(station.platforms[0].name, "A");
    }

    #[test]
    fn test_platform_creation() {
        let platform = Platform { name: "Platform 1".to_string() };
        assert_eq!(platform.name, "Platform 1");
    }
}
