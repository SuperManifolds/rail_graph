/// Migration framework for handling project format version changes
///
/// This module provides a general-purpose migration system that allows
/// incremental migrations between project versions. Each migration is
/// self-contained and can be chained together to migrate from any old
/// version to the current version.
/// Trait for implementing migrations between project versions
pub trait Migration: Send + Sync {
    /// The source version this migration migrates from
    fn source_version(&self) -> u32;

    /// The target version this migration migrates to
    fn target_version(&self) -> u32;

    /// Perform the migration on the raw `MessagePack` data
    ///
    /// # Errors
    /// Returns error if migration fails (e.g., invalid data format)
    fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, String>;
}

/// Chain of migrations that can be applied sequentially
pub struct MigrationChain {
    migrations: Vec<Box<dyn Migration>>,
}

impl MigrationChain {
    /// Create a new migration chain
    #[must_use]
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// Add a migration to the chain
    pub fn add_migration(&mut self, migration: Box<dyn Migration>) {
        self.migrations.push(migration);
    }

    /// Apply migrations to upgrade data from `from_version` to `to_version`
    ///
    /// # Errors
    /// Returns error if no migration path exists or if any migration fails
    pub fn migrate(&self, data: &[u8], from_version: u32, to_version: u32) -> Result<Vec<u8>, String> {
        if from_version == to_version {
            return Ok(data.to_vec());
        }

        if from_version > to_version {
            return Err(format!("Cannot downgrade from version {from_version} to {to_version}"));
        }

        // Find migration path
        let mut current_version = from_version;
        let mut current_data = data.to_vec();

        while current_version < to_version {
            // Find migration from current_version to next version
            let migration = self.migrations.iter()
                .find(|m| m.source_version() == current_version)
                .ok_or_else(|| format!("No migration found from version {current_version}"))?;

            let next_version = migration.target_version();
            if next_version <= current_version {
                return Err(format!("Migration from v{current_version} goes to v{next_version}, which is not forward progress"));
            }

            // Apply migration
            current_data = migration.migrate(&current_data)?;
            current_version = next_version;
        }

        Ok(current_data)
    }
}

impl Default for MigrationChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Create the complete migration chain for all known migrations
#[must_use]
pub fn create_migration_chain() -> MigrationChain {
    let mut chain = MigrationChain::new();

    // Register all migrations in order
    chain.add_migration(Box::new(crate::storage::migrations::v1_to_v2::V1ToV2Migration));

    chain
}

pub mod v1_to_v2;
