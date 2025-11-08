use serde::{Deserialize, Serialize};
use petgraph::stable_graph::EdgeIndex;
use crate::components::infrastructure_canvas::station_renderer::LabelPosition;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Junction {
    pub name: Option<String>,
    #[serde(default)]
    pub position: Option<(f64, f64)>,
    #[serde(default)]
    pub routing_rules: Vec<RoutingRule>,
    #[serde(default)]
    pub label_position: Option<LabelPosition>,
}

impl Junction {
    /// Check if a routing from one edge to another is allowed
    /// Returns true if explicitly allowed, or if no rule exists (default allow)
    #[must_use]
    pub fn is_routing_allowed(&self, from_edge: EdgeIndex, to_edge: EdgeIndex) -> bool {
        // Don't allow routing from an edge to itself
        if from_edge == to_edge {
            return false;
        }

        // Find matching rule
        self.routing_rules
            .iter()
            .find(|rule| rule.from_edge == from_edge && rule.to_edge == to_edge)
            .is_none_or(|rule| rule.allowed) // Default to allowed if no rule exists
    }

    /// Add or update a routing rule
    pub fn set_routing_rule(&mut self, from_edge: EdgeIndex, to_edge: EdgeIndex, allowed: bool) {
        // Don't allow setting rules for same edge
        if from_edge == to_edge {
            return;
        }

        // Find existing rule and update, or add new one
        if let Some(rule) = self.routing_rules
            .iter_mut()
            .find(|rule| rule.from_edge == from_edge && rule.to_edge == to_edge)
        {
            rule.allowed = allowed;
        } else {
            self.routing_rules.push(RoutingRule {
                from_edge,
                to_edge,
                allowed,
            });
        }
    }

    /// Remove a routing rule (returns to default allow behavior)
    pub fn remove_routing_rule(&mut self, from_edge: EdgeIndex, to_edge: EdgeIndex) {
        self.routing_rules.retain(|rule| {
            rule.from_edge != from_edge || rule.to_edge != to_edge
        });
    }

    /// Get all allowed outgoing edges from a given incoming edge
    #[must_use]
    pub fn get_allowed_outgoing_edges(&self, from_edge: EdgeIndex, all_edges: &[EdgeIndex]) -> Vec<EdgeIndex> {
        all_edges
            .iter()
            .filter(|&&to_edge| self.is_routing_allowed(from_edge, to_edge))
            .copied()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    #[serde(with = "edge_index_serde")]
    pub from_edge: EdgeIndex,
    #[serde(with = "edge_index_serde")]
    pub to_edge: EdgeIndex,
    #[serde(default)]
    pub allowed: bool,
}

mod edge_index_serde {
    use petgraph::stable_graph::EdgeIndex;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(edge: &EdgeIndex, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let index_u32 = u32::try_from(edge.index()).unwrap_or(u32::MAX);
        serializer.serialize_u32(index_u32)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<EdgeIndex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let index = u32::deserialize(deserializer)?;
        Ok(EdgeIndex::new(index as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_junction_creation() {
        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: Some((10.0, 20.0)),
            routing_rules: vec![],
            label_position: None,
        };

        assert_eq!(junction.name, Some("Test Junction".to_string()));
        assert_eq!(junction.position, Some((10.0, 20.0)));
        assert_eq!(junction.routing_rules.len(), 0);
    }

    #[test]
    fn test_routing_rule_creation() {
        let rule = RoutingRule {
            from_edge: EdgeIndex::new(0),
            to_edge: EdgeIndex::new(1),
            allowed: true,
        };

        assert_eq!(rule.from_edge.index(), 0);
        assert_eq!(rule.to_edge.index(), 1);
        assert!(rule.allowed);
    }

    #[test]
    fn test_is_routing_allowed_default() {
        let junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };

        // By default, all routings are allowed (except same edge)
        assert!(junction.is_routing_allowed(EdgeIndex::new(0), EdgeIndex::new(1)));
        assert!(junction.is_routing_allowed(EdgeIndex::new(1), EdgeIndex::new(0)));

        // Same edge routing is never allowed
        assert!(!junction.is_routing_allowed(EdgeIndex::new(0), EdgeIndex::new(0)));
    }

    #[test]
    fn test_is_routing_allowed_with_rules() {
        let mut junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };

        // Add a rule forbidding 0->1
        junction.set_routing_rule(EdgeIndex::new(0), EdgeIndex::new(1), false);

        assert!(!junction.is_routing_allowed(EdgeIndex::new(0), EdgeIndex::new(1)));
        assert!(junction.is_routing_allowed(EdgeIndex::new(1), EdgeIndex::new(0))); // Asymmetric
        assert!(junction.is_routing_allowed(EdgeIndex::new(0), EdgeIndex::new(2))); // Other routes still allowed
    }

    #[test]
    fn test_set_routing_rule() {
        let mut junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };

        junction.set_routing_rule(EdgeIndex::new(0), EdgeIndex::new(1), false);
        assert_eq!(junction.routing_rules.len(), 1);
        assert!(!junction.routing_rules[0].allowed);

        // Update existing rule
        junction.set_routing_rule(EdgeIndex::new(0), EdgeIndex::new(1), true);
        assert_eq!(junction.routing_rules.len(), 1);
        assert!(junction.routing_rules[0].allowed);

        // Add different rule
        junction.set_routing_rule(EdgeIndex::new(1), EdgeIndex::new(2), false);
        assert_eq!(junction.routing_rules.len(), 2);
    }

    #[test]
    fn test_remove_routing_rule() {
        let mut junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![
                RoutingRule {
                    from_edge: EdgeIndex::new(0),
                    to_edge: EdgeIndex::new(1),
                    allowed: false,
                },
            ],
            label_position: None,
        };

        assert_eq!(junction.routing_rules.len(), 1);
        junction.remove_routing_rule(EdgeIndex::new(0), EdgeIndex::new(1));
        assert_eq!(junction.routing_rules.len(), 0);

        // After removal, default behavior (allow) should apply
        assert!(junction.is_routing_allowed(EdgeIndex::new(0), EdgeIndex::new(1)));
    }

    #[test]
    fn test_get_allowed_outgoing_edges() {
        let mut junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };

        // Forbid 0->1 and 0->2
        junction.set_routing_rule(EdgeIndex::new(0), EdgeIndex::new(1), false);
        junction.set_routing_rule(EdgeIndex::new(0), EdgeIndex::new(2), false);

        let all_edges = vec![
            EdgeIndex::new(0),
            EdgeIndex::new(1),
            EdgeIndex::new(2),
            EdgeIndex::new(3),
        ];

        let allowed = junction.get_allowed_outgoing_edges(EdgeIndex::new(0), &all_edges);

        // Should only allow 0->3 (0->0 is forbidden by default)
        assert_eq!(allowed.len(), 1);
        assert_eq!(allowed[0].index(), 3);
    }

    #[test]
    fn test_asymmetric_routing() {
        let mut junction = Junction {
            name: Some("Test".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };

        // Allow 0->1 but forbid 1->0
        junction.set_routing_rule(EdgeIndex::new(1), EdgeIndex::new(0), false);

        assert!(junction.is_routing_allowed(EdgeIndex::new(0), EdgeIndex::new(1)));
        assert!(!junction.is_routing_allowed(EdgeIndex::new(1), EdgeIndex::new(0)));
    }
}
