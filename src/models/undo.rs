use super::{RailwayGraph, Line};

/// A snapshot of the application state for undo/redo functionality
#[derive(Clone)]
pub struct UndoSnapshot {
    pub graph: RailwayGraph,
    pub lines: Vec<Line>,
}

impl UndoSnapshot {
    #[must_use]
    pub fn new(graph: RailwayGraph, lines: Vec<Line>) -> Self {
        Self {
            graph,
            lines,
        }
    }
}

/// Manages undo/redo stacks with a configurable maximum depth
#[derive(Clone)]
pub struct UndoManager {
    undo_stack: Vec<UndoSnapshot>,
    redo_stack: Vec<UndoSnapshot>,
    max_levels: usize,
}

impl UndoManager {
    /// Create a new `UndoManager` with the specified maximum undo levels
    #[must_use]
    pub fn new(max_levels: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_levels,
        }
    }

    /// Push a new snapshot onto the undo stack
    /// This clears the redo stack and enforces the maximum depth limit
    pub fn push_snapshot(&mut self, snapshot: UndoSnapshot) {
        // Clear redo stack when new changes are made
        self.redo_stack.clear();

        // Add to undo stack
        self.undo_stack.push(snapshot);

        // Enforce maximum depth (FIFO eviction)
        if self.undo_stack.len() > self.max_levels {
            self.undo_stack.remove(0);
        }
    }

    /// Perform an undo operation, returning the previous snapshot if available
    /// The current state should be provided to push onto the redo stack
    pub fn undo(&mut self, current_snapshot: UndoSnapshot) -> Option<UndoSnapshot> {
        // The last item in undo_stack is the current state (since we record after changes)
        // Pop it to discard it
        self.undo_stack.pop()?;

        // Now pop again to get the actual previous state
        if let Some(previous_snapshot) = self.undo_stack.pop() {
            // Push current state to redo stack
            self.redo_stack.push(current_snapshot);

            // Enforce maximum depth on redo stack
            if self.redo_stack.len() > self.max_levels {
                self.redo_stack.remove(0);
            }

            Some(previous_snapshot)
        } else {
            None
        }
    }

    /// Perform a redo operation, returning the next snapshot if available
    /// The current state should be provided to push onto the undo stack
    pub fn redo(&mut self, current_snapshot: UndoSnapshot) -> Option<UndoSnapshot> {
        if let Some(snapshot) = self.redo_stack.pop() {
            // Push current state to undo stack
            self.undo_stack.push(current_snapshot);

            // Enforce maximum depth on undo stack
            if self.undo_stack.len() > self.max_levels {
                self.undo_stack.remove(0);
            }

            Some(snapshot)
        } else {
            None
        }
    }

    /// Check if undo is available
    /// Need at least 2 items: current state + previous state to restore
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.undo_stack.len() >= 2
    }

    /// Check if redo is available
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all undo/redo history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Get the number of available undo levels
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of available redo levels
    #[must_use]
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new(20)
    }
}
