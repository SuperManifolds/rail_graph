use serde::{Deserialize, Serialize};
use super::station::StationNode;
use super::junction::Junction;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Node {
    Station(StationNode),
    Junction(Junction),
}

impl Node {
    #[must_use]
    pub fn position(&self) -> Option<(f64, f64)> {
        match self {
            Node::Station(s) => s.position,
            Node::Junction(j) => j.position,
        }
    }

    pub fn set_position(&mut self, pos: Option<(f64, f64)>) {
        match self {
            Node::Station(s) => s.position = pos,
            Node::Junction(j) => j.position = pos,
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Node::Station(s) => s.name.clone(),
            Node::Junction(j) => j.name.clone().unwrap_or_else(|| "Junction".to_string()),
        }
    }

    #[must_use]
    pub fn is_station(&self) -> bool {
        matches!(self, Node::Station(_))
    }

    #[must_use]
    pub fn is_junction(&self) -> bool {
        matches!(self, Node::Junction(_))
    }

    #[must_use]
    pub fn as_station(&self) -> Option<&StationNode> {
        match self {
            Node::Station(s) => Some(s),
            Node::Junction(_) => None,
        }
    }

    #[must_use]
    pub fn as_junction(&self) -> Option<&Junction> {
        match self {
            Node::Station(_) => None,
            Node::Junction(j) => Some(j),
        }
    }

    pub fn as_station_mut(&mut self) -> Option<&mut StationNode> {
        match self {
            Node::Station(s) => Some(s),
            Node::Junction(_) => None,
        }
    }

    pub fn as_junction_mut(&mut self) -> Option<&mut Junction> {
        match self {
            Node::Station(_) => None,
            Node::Junction(j) => Some(j),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::station::default_platforms;

    #[test]
    fn test_station_node_wrapper() {
        let station = StationNode {
            name: "Test Station".to_string(),
            position: Some((10.0, 20.0)),
            passing_loop: false,
            platforms: default_platforms(),
            label_position: None,
            external_id: None,
        };
        let node = Node::Station(station);

        assert!(node.is_station());
        assert!(!node.is_junction());
        assert_eq!(node.display_name(), "Test Station");
        assert_eq!(node.position(), Some((10.0, 20.0)));
    }

    #[test]
    fn test_junction_node_wrapper() {
        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: Some((30.0, 40.0)),
            routing_rules: vec![],
            label_position: None,
            external_id: None,
        };
        let node = Node::Junction(junction);

        assert!(!node.is_station());
        assert!(node.is_junction());
        assert_eq!(node.display_name(), "Test Junction");
        assert_eq!(node.position(), Some((30.0, 40.0)));
    }

    #[test]
    fn test_junction_without_name() {
        let junction = Junction {
            name: None,
            position: None,
            routing_rules: vec![],
            label_position: None,
            external_id: None,
        };
        let node = Node::Junction(junction);

        assert_eq!(node.display_name(), "Junction");
    }

    #[test]
    fn test_set_position() {
        let station = StationNode {
            name: "Test".to_string(),
            position: None,
            passing_loop: false,
            platforms: default_platforms(),
            label_position: None,
            external_id: None,
        };
        let mut node = Node::Station(station);

        node.set_position(Some((5.0, 10.0)));
        assert_eq!(node.position(), Some((5.0, 10.0)));
    }

    #[test]
    fn test_as_station() {
        let station = StationNode {
            name: "Test".to_string(),
            position: None,
            passing_loop: false,
            platforms: default_platforms(),
            label_position: None,
            external_id: None,
        };
        let node = Node::Station(station);

        assert!(node.as_station().is_some());
        assert!(node.as_junction().is_none());
    }

    #[test]
    fn test_as_junction() {
        let junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
            external_id: None,
        };
        let node = Node::Junction(junction);

        assert!(node.as_junction().is_some());
        assert!(node.as_station().is_none());
    }
}
