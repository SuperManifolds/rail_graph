/// ID generation utilities
///
/// This module provides functions for generating identifiers:
/// - u64 IDs for most entities (lines, folders, views, etc.)
/// - UUID strings for project IDs (to maintain compatibility with `IndexedDB` keys)
use rand::Rng;

/// Generate a new random u64 ID
///
/// Uses cryptographically secure random number generation to ensure
/// IDs are unpredictable and have minimal collision risk.
#[must_use]
pub fn generate_id() -> u64 {
    rand::thread_rng().gen()
}

/// Generate a new random u64 ID (serde default function)
///
/// This is a convenience wrapper for use with serde's `#[serde(default = "...")]`
#[must_use]
pub fn generate_id_default() -> u64 {
    generate_id()
}

/// Generate a new UUID string for project IDs
#[must_use]
pub fn generate_project_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_generate_id_produces_different_values() {
        let id1 = generate_id();
        let id2 = generate_id();
        let id3 = generate_id();

        // Very unlikely to be equal (1 in 2^64 chance per pair)
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_generate_many_unique_ids() {
        let mut ids = HashSet::new();
        let count = 10_000;

        for _ in 0..count {
            ids.insert(generate_id());
        }

        // All IDs should be unique
        assert_eq!(ids.len(), count);
    }
}
