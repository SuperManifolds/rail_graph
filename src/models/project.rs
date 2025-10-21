use serde::{Deserialize, Serialize};
use super::{Line, RailwayGraph, GraphView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpacingMode {
    Equal,
    DistanceBased,
}

impl Default for SpacingMode {
    fn default() -> Self {
        Self::Equal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Legend {
    pub show_conflicts: bool,
    pub show_line_blocks: bool,
    #[serde(default)]
    pub spacing_mode: SpacingMode,
}

impl Default for Legend {
    fn default() -> Self {
        Self {
            show_conflicts: true,
            show_line_blocks: false,
            spacing_mode: SpacingMode::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(flatten)]
    pub metadata: ProjectMetadata,
    pub lines: Vec<Line>,
    pub graph: RailwayGraph,
    #[serde(default)]
    pub legend: Legend,
    #[serde(default)]
    pub views: Vec<GraphView>,
    #[serde(default)]
    pub active_tab_id: Option<String>,
}

impl Project {
    #[must_use]
    pub fn empty() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Untitled Project".to_string(),
                created_at: now.clone(),
                updated_at: now,
            },
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            legend: Legend::default(),
            views: Vec::new(),
            active_tab_id: None,
        }
    }

    #[must_use]
    pub fn new(lines: Vec<Line>, graph: RailwayGraph, legend: Legend) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Untitled Project".to_string(),
                created_at: now.clone(),
                updated_at: now,
            },
            lines,
            graph,
            legend,
            views: Vec::new(),
            active_tab_id: None,
        }
    }

    #[must_use]
    pub fn new_with_name(name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                created_at: now.clone(),
                updated_at: now,
            },
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            legend: Legend::default(),
            views: Vec::new(),
            active_tab_id: None,
        }
    }

    pub fn touch_updated_at(&mut self) {
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    #[must_use]
    pub fn duplicate_with_name(&self, new_name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: new_name,
                created_at: now.clone(),
                updated_at: now,
            },
            lines: self.lines.clone(),
            graph: self.graph.clone(),
            legend: self.legend.clone(),
            views: self.views.clone(),
            active_tab_id: self.active_tab_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_empty() {
        let project = Project::empty();
        assert_eq!(project.metadata.name, "Untitled Project");
        assert!(project.lines.is_empty());
        assert!(project.views.is_empty());
        assert!(project.active_tab_id.is_none());
    }

    #[test]
    fn test_project_new_with_name() {
        let name = "Test Project".to_string();
        let project = Project::new_with_name(name.clone());
        assert_eq!(project.metadata.name, name);
        assert!(project.lines.is_empty());
        assert!(project.views.is_empty());
    }

    #[test]
    fn test_project_duplicate_with_name() {
        let original = Project::new_with_name("Original".to_string());
        let original_id = original.metadata.id.clone();

        let duplicate = original.duplicate_with_name("Copy".to_string());

        assert_eq!(duplicate.metadata.name, "Copy");
        assert_ne!(duplicate.metadata.id, original_id);
        assert_eq!(duplicate.lines.len(), original.lines.len());
    }

    #[test]
    fn test_touch_updated_at() {
        let mut project = Project::empty();
        let original_updated = project.metadata.updated_at.clone();

        // Sleep a tiny bit to ensure time changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        project.touch_updated_at();
        assert_ne!(project.metadata.updated_at, original_updated);
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = ProjectMetadata {
            id: "test-id".to_string(),
            name: "Test Project".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        };

        // Test serialization round-trip
        let serialized = rmp_serde::to_vec(&metadata).expect("Failed to serialize metadata");
        let deserialized: ProjectMetadata = rmp_serde::from_slice(&serialized).expect("Failed to deserialize metadata");

        assert_eq!(deserialized.id, metadata.id);
        assert_eq!(deserialized.name, metadata.name);
        assert_eq!(deserialized.created_at, metadata.created_at);
        assert_eq!(deserialized.updated_at, metadata.updated_at);
    }

    #[test]
    fn test_project_flattened_metadata() {
        let project = Project::new_with_name("Test".to_string());

        // Serialize the full project
        let serialized = rmp_serde::to_vec(&project).expect("Failed to serialize project");

        // Deserialize into just metadata (this tests the flatten optimization)
        let metadata: ProjectMetadata = rmp_serde::from_slice(&serialized).expect("Failed to deserialize metadata from project");

        assert_eq!(metadata.id, project.metadata.id);
        assert_eq!(metadata.name, project.metadata.name);
        assert_eq!(metadata.created_at, project.metadata.created_at);
        assert_eq!(metadata.updated_at, project.metadata.updated_at);
    }

    #[test]
    fn test_project_serialization_round_trip() {
        let original = Project::new_with_name("Round Trip Test".to_string());

        // Serialize
        let serialized = rmp_serde::to_vec(&original).expect("Failed to serialize project");

        // Deserialize
        let deserialized: Project = rmp_serde::from_slice(&serialized).expect("Failed to deserialize project");

        assert_eq!(deserialized.metadata.id, original.metadata.id);
        assert_eq!(deserialized.metadata.name, original.metadata.name);
        assert_eq!(deserialized.metadata.created_at, original.metadata.created_at);
        assert_eq!(deserialized.metadata.updated_at, original.metadata.updated_at);
        assert_eq!(deserialized.lines.len(), original.lines.len());
        assert_eq!(deserialized.views.len(), original.views.len());
    }
}
