use leptos::{WriteSignal, SignalSet};
use crate::conflict::{Conflict, SerializableConflictContext};
use crate::train_journey::TrainJourney;
use crate::models::{Line, Project, ProjectSettings};

/// Synchronous version of `ConflictDetector` for non-wasm32 targets (tests, etc.)
/// This version deserializes project bytes and generates journeys locally.
pub struct ConflictDetector {
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_is_calculating: WriteSignal<bool>,
}

impl ConflictDetector {
    #[must_use]
    pub fn new(set_conflicts: WriteSignal<Vec<Conflict>>, set_is_calculating: WriteSignal<bool>) -> Self {
        Self { set_conflicts, set_is_calculating }
    }

    /// Detect conflicts synchronously.
    /// Deserializes project from bytes and generates journeys locally.
    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(
        &mut self,
        project_bytes: Vec<u8>,
        lines: Vec<Line>,
        settings: ProjectSettings,
        day_filter: Option<chrono::Weekday>,
    ) {
        // Skip if no lines to check
        if lines.is_empty() {
            self.set_conflicts.set(vec![]);
            self.set_is_calculating.set(false);
            return;
        }

        self.set_is_calculating.set(true);

        // Deserialize project from bytes
        let Ok(project) = Project::from_bytes(&project_bytes) else {
            self.set_conflicts.set(vec![]);
            self.set_is_calculating.set(false);
            return;
        };

        // Filter to visible lines
        let visible_line_ids: std::collections::HashSet<_> = lines.iter().map(|l| l.id).collect();
        let visible_lines: Vec<_> = project.lines
            .into_iter()
            .filter(|line| visible_line_ids.contains(&line.id))
            .collect();

        // Generate journeys
        let journeys = TrainJourney::generate_journeys(&visible_lines, &project.graph, day_filter);
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();

        // Build serializable context from graph
        let station_indices = project.graph.graph.node_indices()
            .enumerate()
            .map(|(idx, node_idx)| (node_idx, idx))
            .collect();
        let context = SerializableConflictContext::from_graph(
            &project.graph,
            station_indices,
            settings.station_margin,
            settings.minimum_separation,
            settings.ignore_same_direction_platform_conflicts,
        );

        let (conflicts, _) = crate::conflict::detect_line_conflicts(&journeys_vec, &context);
        self.set_conflicts.set(conflicts);
        self.set_is_calculating.set(false);
    }
}
