use petgraph::stable_graph::NodeIndex;
use super::RailwayGraph;
use super::tracks::Tracks;
use crate::models::junction::Junction;
use crate::models::node::Node;

/// Extension trait for junction-related operations on `RailwayGraph`
pub trait Junctions {
    /// Add a junction node and return its `NodeIndex`
    fn add_junction(&mut self, junction: Junction) -> NodeIndex;

    /// Get junction by `NodeIndex`
    fn get_junction(&self, index: NodeIndex) -> Option<&Junction>;

    /// Get mutable junction by `NodeIndex`
    fn get_junction_mut(&mut self, index: NodeIndex) -> Option<&mut Junction>;

    /// Check if a node is a junction
    fn is_junction(&self, index: NodeIndex) -> bool;

    /// Delete a junction (removes the node and all connected edges)
    /// Returns the list of removed edge indices
    fn delete_junction(&mut self, index: NodeIndex) -> Vec<usize>;

    /// Validate that a route respects junction routing rules
    ///
    /// # Errors
    ///
    /// Returns an error if any consecutive pair of route segments violates a junction routing rule
    fn validate_route_through_junctions(&self, route: &[crate::models::RouteSegment]) -> Result<(), String>;

    /// Validate a junction's configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the junction has insufficient connections or invalid routing rules
    fn validate_junction(&self, index: NodeIndex) -> Result<(), String>;

    /// Interpolate junction position based on connected stations
    ///
    /// If the junction has no position set or if `force` is true, calculates a position
    /// by averaging the positions of connected stations. Returns true if position was updated.
    fn interpolate_junction_position(&mut self, index: NodeIndex, force: bool) -> bool;
}

impl Junctions for RailwayGraph {
    fn add_junction(&mut self, junction: Junction) -> NodeIndex {
        self.graph.add_node(Node::Junction(junction))
    }

    fn get_junction(&self, index: NodeIndex) -> Option<&Junction> {
        self.graph.node_weight(index).and_then(|node| node.as_junction())
    }

    fn get_junction_mut(&mut self, index: NodeIndex) -> Option<&mut Junction> {
        self.graph.node_weight_mut(index).and_then(|node| node.as_junction_mut())
    }

    fn is_junction(&self, index: NodeIndex) -> bool {
        self.graph.node_weight(index).is_some_and(Node::is_junction)
    }

    fn delete_junction(&mut self, index: NodeIndex) -> Vec<usize> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        // Get all connected edges with their node information
        let mut edges: Vec<(usize, NodeIndex, NodeIndex, Vec<crate::models::Track>)> = Vec::new();

        // Outgoing edges from the junction
        for e in self.graph.edges(index) {
            edges.push((e.id().index(), e.source(), e.target(), e.weight().tracks.clone()));
        }

        // Incoming edges to the junction
        for e in self.graph.edges_directed(index, Direction::Incoming) {
            edges.push((e.id().index(), e.source(), e.target(), e.weight().tracks.clone()));
        }

        let removed_edge_indices: Vec<usize> = edges.iter().map(|(idx, _, _, _)| *idx).collect();

        // If this is a "through" junction with exactly 2 connections, restore the direct edge
        if edges.len() == 2 {
            // Collect the two endpoints (nodes that are NOT the junction)
            let mut connected_nodes = Vec::new();
            for (_, from, to, _) in &edges {
                if *from != index {
                    connected_nodes.push(*from);
                }
                if *to != index {
                    connected_nodes.push(*to);
                }
            }

            // We should have exactly 2 endpoints
            if connected_nodes.len() == 2 {
                let node1 = connected_nodes[0];
                let node2 = connected_nodes[1];
                let tracks = edges[0].3.clone();

                // Remove the junction node (this also removes all connected edges)
                self.graph.remove_node(index);

                // Create a new direct edge between the two endpoints
                self.add_track(node1, node2, tracks);
            } else {
                // Safety fallback: just remove the junction
                self.graph.remove_node(index);
            }
        } else {
            // For junctions with != 2 connections, just remove it
            self.graph.remove_node(index);
        }

        removed_edge_indices
    }

    fn validate_route_through_junctions(&self, route: &[crate::models::RouteSegment]) -> Result<(), String> {
        use petgraph::stable_graph::EdgeIndex;

        // Check each consecutive pair of segments
        for i in 0..route.len().saturating_sub(1) {
            let current_segment = &route[i];
            let next_segment = &route[i + 1];

            let current_edge = EdgeIndex::new(current_segment.edge_index);
            let next_edge = EdgeIndex::new(next_segment.edge_index);

            // Get the connecting node between these two segments
            let Some((_, current_to)) = self.get_track_endpoints(current_edge) else {
                continue;
            };

            let Some((next_from, _)) = self.get_track_endpoints(next_edge) else {
                continue;
            };

            // If they don't connect at the same node, skip
            if current_to != next_from {
                continue;
            }

            // Check if the connecting node is a junction
            if !self.is_junction(current_to) {
                continue;
            }

            // Get the junction and check routing rules
            if let Some(junction) = self.get_junction(current_to) {
                if !junction.is_routing_allowed(current_edge, next_edge) {
                    let junction_name = junction.name.clone()
                        .unwrap_or_else(|| format!("Junction at {current_to:?}"));

                    return Err(format!(
                        "Routing from edge {} to edge {} is not allowed at {}",
                        current_edge.index(),
                        next_edge.index(),
                        junction_name
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_junction(&self, index: NodeIndex) -> Result<(), String> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        // Check that the node is actually a junction
        if !self.is_junction(index) {
            return Err("Node is not a junction".to_string());
        }

        // Get all connected edges (incoming and outgoing)
        let mut all_edges = Vec::new();
        for edge in self.graph.edges(index) {
            all_edges.push(edge.id());
        }
        for edge in self.graph.edges_directed(index, Direction::Incoming) {
            all_edges.push(edge.id());
        }

        // Minimum connection requirement: junctions must have at least 3 connections
        if all_edges.len() < 3 {
            let junction_name = self.get_junction(index)
                .and_then(|j| j.name.as_ref())
                .map_or_else(|| format!("Junction at {index:?}"), std::clone::Clone::clone);
            return Err(format!("{junction_name} has only {} connection(s), minimum is 3", all_edges.len()));
        }

        // Validate routing rules
        if let Some(junction) = self.get_junction(index) {
            // Check that all routing rules reference valid edges
            for rule in &junction.routing_rules {
                if !all_edges.contains(&rule.from_edge) {
                    return Err(format!("Routing rule references non-existent from_edge: {}", rule.from_edge.index()));
                }
                if !all_edges.contains(&rule.to_edge) {
                    return Err(format!("Routing rule references non-existent to_edge: {}", rule.to_edge.index()));
                }
            }

            // Check for dead ends: ensure at least one outgoing route is possible from each incoming edge
            for &from_edge in &all_edges {
                let allowed_outgoing = junction.get_allowed_outgoing_edges(from_edge, &all_edges);
                if allowed_outgoing.is_empty() {
                    let junction_name = junction.name.as_ref()
                        .map_or_else(|| format!("Junction at {index:?}"), std::clone::Clone::clone);
                    return Err(format!(
                        "{junction_name}: edge {} has no allowed outgoing routes (creates a dead end)",
                        from_edge.index()
                    ));
                }
            }
        }

        Ok(())
    }

    fn interpolate_junction_position(&mut self, index: NodeIndex, force: bool) -> bool {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;
        use super::stations::Stations;

        // Check if the node is a junction
        if !self.is_junction(index) {
            return false;
        }

        // If junction already has a position and we're not forcing update, skip
        if !force && self.get_station_position(index).is_some() {
            return false;
        }

        // Collect positions of all connected nodes (stations or junctions)
        let mut connected_positions = Vec::new();

        // Outgoing edges
        for edge in self.graph.edges(index) {
            if let Some(pos) = self.get_station_position(edge.target()) {
                connected_positions.push(pos);
            }
        }

        // Incoming edges
        for edge in self.graph.edges_directed(index, Direction::Incoming) {
            if let Some(pos) = self.get_station_position(edge.source()) {
                connected_positions.push(pos);
            }
        }

        // Need at least one positioned neighbor to interpolate
        if connected_positions.is_empty() {
            return false;
        }

        // Calculate average position
        let sum_x: f64 = connected_positions.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = connected_positions.iter().map(|(_, y)| y).sum();
        #[allow(clippy::cast_precision_loss)]
        let count = connected_positions.len() as f64;

        let interpolated_pos = (sum_x / count, sum_y / count);
        self.set_station_position(index, interpolated_pos);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RailwayGraph;
    use crate::models::railway_graph::stations::Stations;
    use crate::models::railway_graph::tracks::Tracks;
    use crate::models::track::{Track, TrackDirection};

    #[test]
    fn test_add_junction() {
        let mut graph = RailwayGraph::new();
        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: Some((10.0, 20.0)),
            routing_rules: vec![],
            label_position: None,
        };

        let idx = graph.add_junction(junction);
        assert_eq!(graph.graph.node_count(), 1);
        assert!(graph.is_junction(idx));
    }

    #[test]
    fn test_get_junction() {
        let mut graph = RailwayGraph::new();
        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: Some((10.0, 20.0)),
            routing_rules: vec![],
            label_position: None,
        };

        let idx = graph.add_junction(junction);
        let retrieved = graph.get_junction(idx);

        assert!(retrieved.is_some());
        assert_eq!(retrieved.expect("Junction should exist").name, Some("Test Junction".to_string()));
    }

    #[test]
    fn test_is_junction() {
        let mut graph = RailwayGraph::new();

        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };
        let j_idx = graph.add_junction(junction);

        let s_idx = graph.add_or_get_station("Station A".to_string());

        assert!(graph.is_junction(j_idx));
        assert!(!graph.is_junction(s_idx));
    }

    #[test]
    fn test_delete_junction() {
        let mut graph = RailwayGraph::new();

        let s1 = graph.add_or_get_station("Station A".to_string());
        let s2 = graph.add_or_get_station("Station B".to_string());

        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };
        let j_idx = graph.add_junction(junction);

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        graph.add_track(s1, j_idx, tracks.clone());
        graph.add_track(j_idx, s2, tracks);

        assert_eq!(graph.graph.node_count(), 3);
        assert_eq!(graph.graph.edge_count(), 2);

        let removed_edges = graph.delete_junction(j_idx);

        // With StableGraph, junction with 2 connections is a "through" junction
        // Deleting it restores the direct edge between the two endpoints
        assert_eq!(graph.graph.node_count(), 2); // Only 2 valid nodes remain
        assert_eq!(graph.graph.edge_count(), 1); // 1 edge remains (the restored direct edge)
        assert_eq!(removed_edges.len(), 2);

        // Verify a direct edge exists between s1 and s2 (in either direction)
        let has_edge = graph.graph.find_edge(s1, s2).is_some() || graph.graph.find_edge(s2, s1).is_some();
        assert!(has_edge, "Expected an edge between s1 and s2 in either direction");
    }

    #[test]
    fn test_get_junction_mut() {
        let mut graph = RailwayGraph::new();
        let junction = Junction {
            name: Some("Original Name".to_string()),
            position: Some((10.0, 20.0)),
            routing_rules: vec![],
            label_position: None,
        };

        let idx = graph.add_junction(junction);

        if let Some(junction) = graph.get_junction_mut(idx) {
            junction.name = Some("Modified Name".to_string());
            junction.position = Some((30.0, 40.0));
        }

        let retrieved = graph.get_junction(idx).expect("Junction should exist");
        assert_eq!(retrieved.name, Some("Modified Name".to_string()));
        assert_eq!(retrieved.position, Some((30.0, 40.0)));
    }

    #[test]
    fn test_junction_position_via_node() {
        let mut graph = RailwayGraph::new();
        let junction = Junction {
            name: Some("Test Junction".to_string()),
            position: Some((50.0, 60.0)),
            routing_rules: vec![],
            label_position: None,
        };

        let idx = graph.add_junction(junction);

        // Test getting position via station position method (should work for junctions too)
        assert_eq!(graph.get_station_position(idx), Some((50.0, 60.0)));

        // Test setting position via station position method
        graph.set_station_position(idx, (100.0, 200.0));
        assert_eq!(graph.get_station_position(idx), Some((100.0, 200.0)));
    }

    #[test]
    fn test_junction_with_multiple_connections() {
        let mut graph = RailwayGraph::new();

        let junction = Junction {
            name: Some("Central Junction".to_string()),
            position: Some((0.0, 0.0)),
            routing_rules: vec![],
            label_position: None,
        };
        let j_idx = graph.add_junction(junction);

        // Create a junction with 4 connected stations
        let s1 = graph.add_or_get_station("North Station".to_string());
        let s2 = graph.add_or_get_station("South Station".to_string());
        let s3 = graph.add_or_get_station("East Station".to_string());
        let s4 = graph.add_or_get_station("West Station".to_string());

        graph.add_track(s1, j_idx, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(s2, j_idx, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j_idx, s3, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j_idx, s4, vec![Track { direction: TrackDirection::Bidirectional }]);

        assert_eq!(graph.graph.node_count(), 5);
        assert_eq!(graph.graph.edge_count(), 4);

        // Delete junction should remove all 4 edges
        let removed_edges = graph.delete_junction(j_idx);
        assert_eq!(removed_edges.len(), 4);
        assert_eq!(graph.graph.node_count(), 4);
        assert_eq!(graph.graph.edge_count(), 0);
    }

    #[test]
    fn test_junction_without_name() {
        let mut graph = RailwayGraph::new();
        let junction = Junction {
            name: None,
            position: Some((10.0, 20.0)),
            routing_rules: vec![],
            label_position: None,
        };

        let idx = graph.add_junction(junction);
        let retrieved = graph.get_junction(idx).expect("Junction should exist");

        assert!(retrieved.name.is_none());
        assert_eq!(retrieved.position, Some((10.0, 20.0)));
    }

    #[test]
    fn test_mixed_network_with_junctions_and_stations() {
        let mut graph = RailwayGraph::new();

        // Create a network: Station A -> Junction -> Station B -> Station C
        let s1 = graph.add_or_get_station("Station A".to_string());
        let junction = Junction {
            name: Some("Junction 1".to_string()),
            position: Some((50.0, 50.0)),
            routing_rules: vec![],
            label_position: None,
        };
        let j1 = graph.add_junction(junction);
        let s2 = graph.add_or_get_station("Station B".to_string());
        let s3 = graph.add_or_get_station("Station C".to_string());

        graph.add_track(s1, j1, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j1, s2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(s2, s3, vec![Track { direction: TrackDirection::Bidirectional }]);

        assert_eq!(graph.graph.node_count(), 4);
        assert_eq!(graph.graph.edge_count(), 3);

        // Verify types
        assert!(!graph.is_junction(s1));
        assert!(graph.is_junction(j1));
        assert!(!graph.is_junction(s2));
        assert!(!graph.is_junction(s3));

        // Verify get_station_name doesn't return names for junctions
        assert_eq!(graph.get_station_name(s1), Some("Station A"));
        assert_eq!(graph.get_station_name(j1), None);
        assert_eq!(graph.get_station_name(s2), Some("Station B"));
    }

    #[test]
    fn test_validate_route_allowed() {
        use crate::models::RouteSegment;
        use chrono::Duration;

        let mut graph = RailwayGraph::new();

        // Create: A -> Junction -> B
        let s_a = graph.add_or_get_station("A".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });
        let s_b = graph.add_or_get_station("B".to_string());

        let e1 = graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            RouteSegment {
                edge_index: e1.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(0),
            },
            RouteSegment {
                edge_index: e2.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(30),
            },
        ];

        // No routing rules, so should be allowed
        assert!(graph.validate_route_through_junctions(&route).is_ok());
    }

    #[test]
    fn test_validate_route_forbidden() {
        use crate::models::RouteSegment;
        use chrono::Duration;

        let mut graph = RailwayGraph::new();

        // Create: A -> Junction -> B
        let s_a = graph.add_or_get_station("A".to_string());
        let mut junction = Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };
        let j = graph.add_junction(junction.clone());
        let s_b = graph.add_or_get_station("B".to_string());

        let e1 = graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Add routing rule forbidding e1 -> e2
        junction.set_routing_rule(e1, e2, false);

        // Update junction in graph
        if let Some(crate::models::Node::Junction(ref mut j_mut)) = graph.graph.node_weight_mut(j) {
            *j_mut = junction;
        }

        let route = vec![
            RouteSegment {
                edge_index: e1.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(0),
            },
            RouteSegment {
                edge_index: e2.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(30),
            },
        ];

        // Should be forbidden
        let result = graph.validate_route_through_junctions(&route);
        assert!(result.is_err());
        assert!(result.expect_err("should be forbidden").contains("not allowed"));
    }

    #[test]
    fn test_validate_junction_valid() {
        let mut graph = RailwayGraph::new();

        // Create a valid junction with 3 connections
        let s_a = graph.add_or_get_station("A".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });
        let s_b = graph.add_or_get_station("B".to_string());
        let s_c = graph.add_or_get_station("C".to_string());

        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Should be valid
        assert!(graph.validate_junction(j).is_ok());
    }

    #[test]
    fn test_validate_junction_insufficient_connections() {
        let mut graph = RailwayGraph::new();

        // Create a junction with only 2 connections (minimum is 3)
        let s_a = graph.add_or_get_station("A".to_string());
        let s_b = graph.add_or_get_station("B".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Insufficient Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });

        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Should fail validation (has 2, needs 3)
        let result = graph.validate_junction(j);
        assert!(result.is_err());
        assert!(result.expect_err("should fail").contains("only 2 connection"));
    }

    #[test]
    fn test_validate_junction_not_a_junction() {
        let mut graph = RailwayGraph::new();

        // Try to validate a station as a junction
        let s = graph.add_or_get_station("Station".to_string());

        let result = graph.validate_junction(s);
        assert!(result.is_err());
        assert!(result.expect_err("should fail").contains("not a junction"));
    }

    #[test]
    fn test_validate_junction_dead_end() {
        let mut graph = RailwayGraph::new();

        // Create junction with routes that create a dead end
        let s_a = graph.add_or_get_station("A".to_string());
        let mut junction = Junction {
            name: Some("Dead End Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };
        let j = graph.add_junction(junction.clone());
        let s_b = graph.add_or_get_station("B".to_string());
        let s_c = graph.add_or_get_station("C".to_string());

        let e1 = graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e3 = graph.add_track(j, s_c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Forbid all routes from e1, creating a dead end
        junction.set_routing_rule(e1, e2, false);
        junction.set_routing_rule(e1, e3, false);

        // Update junction in graph
        if let Some(crate::models::Node::Junction(ref mut j_mut)) = graph.graph.node_weight_mut(j) {
            *j_mut = junction;
        }

        // Should fail validation
        let result = graph.validate_junction(j);
        assert!(result.is_err());
        assert!(result.expect_err("should fail").contains("no allowed outgoing routes"));
    }

    #[test]
    fn test_validate_junction_invalid_edge_reference() {
        use petgraph::stable_graph::EdgeIndex;

        let mut graph = RailwayGraph::new();

        // Create junction with 3 connections but invalid routing rule
        let s_a = graph.add_or_get_station("A".to_string());
        let s_b = graph.add_or_get_station("B".to_string());
        let s_c = graph.add_or_get_station("C".to_string());
        let mut junction = Junction {
            name: Some("Invalid Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };
        let j = graph.add_junction(junction.clone());

        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Add a rule referencing a non-existent edge
        junction.set_routing_rule(EdgeIndex::new(999), EdgeIndex::new(1000), false);

        // Update junction in graph
        if let Some(crate::models::Node::Junction(ref mut j_mut)) = graph.graph.node_weight_mut(j) {
            *j_mut = junction;
        }

        // Should fail validation
        let result = graph.validate_junction(j);
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        assert!(err.contains("non-existent"));
    }

    #[test]
    fn test_validate_route_asymmetric() {
        use crate::models::RouteSegment;
        use chrono::Duration;

        let mut graph = RailwayGraph::new();

        // Create: A <-> Junction <-> B (with edges in both directions)
        let s_a = graph.add_or_get_station("A".to_string());
        let mut junction = Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        };
        let j = graph.add_junction(junction.clone());
        let s_b = graph.add_or_get_station("B".to_string());

        // Create edges in both directions for bidirectional connectivity
        let e1 = graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e1_rev = graph.add_track(j, s_a, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2_rev = graph.add_track(s_b, j, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Forbid only e2_rev -> e1_rev (B -> A direction through junction)
        junction.set_routing_rule(e2_rev, e1_rev, false);

        // Update junction in graph
        if let Some(crate::models::Node::Junction(ref mut j_mut)) = graph.graph.node_weight_mut(j) {
            *j_mut = junction;
        }

        // Forward route (A -> B) should still be allowed
        let forward_route = vec![
            RouteSegment {
                edge_index: e1.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(0),
            },
            RouteSegment {
                edge_index: e2.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(30),
            },
        ];

        assert!(graph.validate_route_through_junctions(&forward_route).is_ok());

        // Reverse route (B -> A) should be forbidden
        let reverse_route = vec![
            RouteSegment {
                edge_index: e2_rev.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(0),
            },
            RouteSegment {
                edge_index: e1_rev.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(30),
            },
        ];

        let result = graph.validate_route_through_junctions(&reverse_route);
        assert!(result.is_err());
    }

    #[test]
    fn test_interpolate_junction_position_between_two_stations() {
        let mut graph = RailwayGraph::new();

        // Create two stations with known positions
        let s_a = graph.add_or_get_station("A".to_string());
        let s_b = graph.add_or_get_station("B".to_string());
        graph.set_station_position(s_a, (0.0, 0.0));
        graph.set_station_position(s_b, (100.0, 100.0));

        // Create junction without position
        let j = graph.add_junction(Junction {
            name: Some("Test Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });

        // Connect junction to both stations
        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Interpolate position
        let updated = graph.interpolate_junction_position(j, false);
        assert!(updated);

        // Should be at midpoint
        let pos = graph.get_station_position(j).expect("Junction should have position");
        assert_eq!(pos, (50.0, 50.0));
    }

    #[test]
    fn test_interpolate_junction_position_with_three_stations() {
        let mut graph = RailwayGraph::new();

        // Create three stations forming a triangle
        let s_a = graph.add_or_get_station("A".to_string());
        let s_b = graph.add_or_get_station("B".to_string());
        let s_c = graph.add_or_get_station("C".to_string());
        graph.set_station_position(s_a, (0.0, 0.0));
        graph.set_station_position(s_b, (60.0, 0.0));
        graph.set_station_position(s_c, (30.0, 60.0));

        // Create junction without position
        let j = graph.add_junction(Junction {
            name: Some("Central Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });

        // Connect junction to all three stations
        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Interpolate position
        let updated = graph.interpolate_junction_position(j, false);
        assert!(updated);

        // Should be at centroid: ((0+60+30)/3, (0+0+60)/3) = (30, 20)
        let pos = graph.get_station_position(j).expect("Junction should have position");
        assert_eq!(pos, (30.0, 20.0));
    }

    #[test]
    fn test_interpolate_junction_position_no_connected_nodes() {
        let mut graph = RailwayGraph::new();

        // Create isolated junction
        let j = graph.add_junction(Junction {
            name: Some("Isolated Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });

        // Should not interpolate (no connected nodes)
        let updated = graph.interpolate_junction_position(j, false);
        assert!(!updated);
        assert_eq!(graph.get_station_position(j), None);
    }

    #[test]
    fn test_interpolate_junction_position_already_has_position() {
        let mut graph = RailwayGraph::new();

        // Create stations
        let s_a = graph.add_or_get_station("A".to_string());
        let s_b = graph.add_or_get_station("B".to_string());
        graph.set_station_position(s_a, (0.0, 0.0));
        graph.set_station_position(s_b, (100.0, 100.0));

        // Create junction with existing position
        let j = graph.add_junction(Junction {
            name: Some("Test Junction".to_string()),
            position: Some((25.0, 75.0)),
            routing_rules: vec![],
            label_position: None,
        });

        // Connect junction to stations
        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Should not interpolate (already has position, force=false)
        let updated = graph.interpolate_junction_position(j, false);
        assert!(!updated);

        // Position should remain unchanged
        let pos = graph.get_station_position(j).expect("Junction should have position");
        assert_eq!(pos, (25.0, 75.0));
    }

    #[test]
    fn test_interpolate_junction_position_force_update() {
        let mut graph = RailwayGraph::new();

        // Create stations
        let s_a = graph.add_or_get_station("A".to_string());
        let s_b = graph.add_or_get_station("B".to_string());
        graph.set_station_position(s_a, (0.0, 0.0));
        graph.set_station_position(s_b, (100.0, 100.0));

        // Create junction with existing position
        let j = graph.add_junction(Junction {
            name: Some("Test Junction".to_string()),
            position: Some((25.0, 75.0)),
            routing_rules: vec![],
            label_position: None,
        });

        // Connect junction to stations
        graph.add_track(s_a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, s_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Should interpolate (force=true)
        let updated = graph.interpolate_junction_position(j, true);
        assert!(updated);

        // Position should be updated to midpoint
        let pos = graph.get_station_position(j).expect("Junction should have position");
        assert_eq!(pos, (50.0, 50.0));
    }

    #[test]
    fn test_interpolate_junction_position_not_a_junction() {
        let mut graph = RailwayGraph::new();

        // Create a station
        let s = graph.add_or_get_station("Station".to_string());

        // Should not interpolate (not a junction)
        let updated = graph.interpolate_junction_position(s, false);
        assert!(!updated);
    }
}
