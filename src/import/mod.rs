pub mod csv;
pub mod jtraingraph;
pub mod shared;

use crate::models::{Line, RailwayGraph, TrackHandedness};

// Re-export commonly used items
pub use csv::{CsvImport, CsvImportConfig, ColumnType, ColumnMapping, ParsedCsv};
pub use jtraingraph::{JTrainGraphImport, JTrainGraphConfig};
pub use shared::{create_tracks_with_count, ensure_platforms_up_to, get_or_add_platform};

/// Result of an import operation containing created lines and metadata
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub lines: Vec<Line>,
    pub stations_added: usize,
    pub edges_added: usize,
}

/// Mode for importing data - either create new infrastructure or use existing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// Create new stations, junctions, and tracks as needed
    CreateInfrastructure,
    /// Use existing infrastructure only, find paths between stations
    UseExisting,
}

/// Main import trait for different file formats
pub trait Import {
    /// Configuration type for this importer
    type Config: Clone;

    /// Intermediate parsed format (before import)
    type Parsed;

    /// Error type for parsing
    type ParseError: std::fmt::Display;

    /// Parse raw content into intermediate format
    ///
    /// # Errors
    /// Returns error if content cannot be parsed into the expected format
    fn parse(content: &str) -> Result<Self::Parsed, Self::ParseError>;

    /// Analyze content and suggest configuration (optional, for interactive imports)
    /// Returns None if this importer doesn't support analysis
    #[must_use]
    fn analyze(content: &str, filename: Option<String>) -> Option<Self::Config> {
        let _ = (content, filename);
        None
    }

    /// Import parsed data into graph with configuration
    ///
    /// # Errors
    /// Returns error if import fails (e.g., invalid data, missing infrastructure in `UseExisting` mode)
    fn import(
        parsed: &Self::Parsed,
        config: &Self::Config,
        mode: ImportMode,
        graph: &mut RailwayGraph,
        existing_line_count: usize,
        existing_line_ids: &[String],
        handedness: TrackHandedness,
    ) -> Result<ImportResult, String>;

    /// Single-step import helper (parse + import)
    ///
    /// # Errors
    /// Returns error if parsing or import fails
    fn import_from_content(
        content: &str,
        config: &Self::Config,
        mode: ImportMode,
        graph: &mut RailwayGraph,
        existing_line_count: usize,
        existing_line_ids: &[String],
        handedness: TrackHandedness,
    ) -> Result<ImportResult, String> {
        let parsed = Self::parse(content)
            .map_err(|e| format!("Parse error: {e}"))?;
        Self::import(&parsed, config, mode, graph, existing_line_count, existing_line_ids, handedness)
    }
}
