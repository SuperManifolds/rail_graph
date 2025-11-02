use gloo_worker::{HandlerId, Worker, WorkerScope};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::import::geojson::{GeoJsonImport, GeoJsonImportRequest, GeoJsonImportResponse, GraphUpdate};
use crate::import::{Import, ImportMode};
use crate::models::{Node, RailwayGraph, TrackHandedness, Stations};

// Re-export BincodeCodec from conflict_worker for consistency
pub use crate::conflict_worker::BincodeCodec;

pub struct GeoJsonImportWorker;

impl Worker for GeoJsonImportWorker {
    type Input = GeoJsonImportRequest;
    type Output = GeoJsonImportResponse;
    type Message = ();

    fn create(_scope: &WorkerScope<Self>) -> Self {
        web_sys::console::log_1(&"GeoJSON import worker created".into());
        Self
    }

    fn update(&mut self, _scope: &WorkerScope<Self>, _msg: Self::Message) {
        // No internal messages needed
    }

    fn received(&mut self, scope: &WorkerScope<Self>, msg: Self::Input, id: HandlerId) {
        web_sys::console::log_1(&"Worker received import request".into());
        let start = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now());

        // Parse GeoJSON string in worker
        web_sys::console::log_1(&format!("Parsing GeoJSON ({} bytes)", msg.geojson_string.len()).into());
        let parsed = match GeoJsonImport::parse(&msg.geojson_string) {
            Ok(p) => p,
            Err(e) => {
                web_sys::console::log_1(&format!("Worker parse failed: {}", e).into());
                scope.respond(
                    id,
                    GeoJsonImportResponse {
                        result: Err(format!("Failed to parse GeoJSON: {}", e)),
                        updates: vec![],
                        stations_added: 0,
                        edges_added: 0,
                    },
                );
                return;
            }
        };
        web_sys::console::log_1(&"GeoJSON parsed successfully".into());

        // Create a temporary graph to perform the import
        let mut temp_graph = RailwayGraph::new();

        // Perform the import
        let result = GeoJsonImport::import(
            &parsed,
            &msg.config,
            ImportMode::CreateInfrastructure,
            &mut temp_graph,
            0,
            &[],
            TrackHandedness::RightHand,
        );

        let (updates, stations_added, edges_added) = match result {
            Ok(import_result) => {
                // Extract GraphUpdate operations from the resulting graph
                let updates = extract_graph_updates(&temp_graph);
                (updates, import_result.stations_added, import_result.edges_added)
            }
            Err(e) => {
                web_sys::console::log_1(&format!("Worker import failed: {}", e).into());
                scope.respond(
                    id,
                    GeoJsonImportResponse {
                        result: Err(e),
                        updates: vec![],
                        stations_added: 0,
                        edges_added: 0,
                    },
                );
                return;
            }
        };

        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            web_sys::console::log_1(
                &format!(
                    "Worker import took {:.2}ms ({} stations, {} edges)",
                    elapsed, stations_added, edges_added
                )
                .into(),
            );
        }

        scope.respond(
            id,
            GeoJsonImportResponse {
                result: Ok(()),
                updates,
                stations_added,
                edges_added,
            },
        );
    }
}

/// Extract GraphUpdate operations from a completed graph import
fn extract_graph_updates(graph: &RailwayGraph) -> Vec<GraphUpdate> {
    let mut updates = Vec::new();

    // Extract station additions with positions
    for node_idx in graph.graph.node_indices() {
        if let Some(Node::Station(station)) = graph.graph.node_weight(node_idx) {
            let position = graph.get_station_position(node_idx).unwrap_or((0.0, 0.0));

            // Use the station's internal ID (from station_name_to_index) as the key
            // We need to find the key that maps to this node_idx
            let station_id = graph
                .station_name_to_index
                .iter()
                .find(|(_, &idx)| idx == node_idx)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| station.name.clone()); // Fallback to name if not found

            updates.push(GraphUpdate::AddStation {
                id: station_id,
                name: station.name.clone(),
                position,
            });
        }
    }

    // Extract edge additions
    for edge_ref in graph.graph.edge_references() {
        let start_idx = edge_ref.source();
        let end_idx = edge_ref.target();
        let edge_weight = edge_ref.weight();

        // Get station IDs for both endpoints
        let start_id = graph
            .station_name_to_index
            .iter()
            .find(|(_, &idx)| idx == start_idx)
            .map(|(id, _)| id.clone())
            .unwrap_or_default();

        let end_id = graph
            .station_name_to_index
            .iter()
            .find(|(_, &idx)| idx == end_idx)
            .map(|(id, _)| id.clone())
            .unwrap_or_default();

        // First track
        if let Some(first_track) = edge_weight.tracks.first() {
            let bidirectional = matches!(
                first_track.direction,
                crate::models::TrackDirection::Bidirectional
            );

            updates.push(GraphUpdate::AddTrack {
                start_id: start_id.clone(),
                end_id: end_id.clone(),
                bidirectional,
            });

            // Additional parallel tracks
            for track in edge_weight.tracks.iter().skip(1) {
                let bidirectional = matches!(
                    track.direction,
                    crate::models::TrackDirection::Bidirectional
                );

                updates.push(GraphUpdate::AddParallelTrack {
                    start_id: start_id.clone(),
                    end_id: end_id.clone(),
                    bidirectional,
                });
            }
        }
    }

    updates
}
