//! Cache version management and migration

use cuenv_core::{Error, Result};
use cuenv_utils::atomic_file::write_atomic_string;
use std::fs;
use std::path::Path;

/// Cache version for migration support
pub const CACHE_VERSION: u32 = 1;

/// Handle cache version checking and migration
pub struct CacheMigrator {
    version: u32,
}

impl CacheMigrator {
    pub fn new() -> Self {
        Self {
            version: CACHE_VERSION,
        }
    }

    /// Check cache version and migrate if necessary
    pub fn check_and_migrate(&self, base_dir: &Path) -> Result<()> {
        let version_file = base_dir.join("VERSION");

        if version_file.exists() {
            let content = fs::read_to_string(&version_file)
                .map_err(|e| Error::file_system(&version_file, "read version file", e))?;

            let file_version: u32 = content
                .trim()
                .parse()
                .map_err(|_| Error::configuration("Invalid cache version format".to_string()))?;

            if file_version < self.version {
                log::info!(
                    "Migrating cache from version {} to {}",
                    file_version,
                    self.version
                );
                self.migrate_cache(base_dir, file_version)?;
            } else if file_version > self.version {
                return Err(Error::configuration(format!(
                    "Cache version {} is newer than supported version {}",
                    file_version, self.version
                )));
            }
        } else {
            // Write current version atomically
            write_atomic_string(&version_file, &self.version.to_string())?;
        }

        Ok(())
    }

    /// Migrate cache from older version
    fn migrate_cache(&self, base_dir: &Path, from_version: u32) -> Result<()> {
        // For now, just clear the cache on migration
        log::warn!("Cache migration: clearing cache due to version change");
        self.clear_cache_for_migration(base_dir)?;

        // Write new version
        let version_file = base_dir.join("VERSION");
        write_atomic_string(&version_file, &self.version.to_string())?;

        // In the future, we could add specific migration logic here
        match from_version {
            0 => {
                // Migration from version 0 to current
                log::info!("Migrating from version 0");
            }
            _ => {
                // Generic migration
                log::info!("Generic migration from version {}", from_version);
            }
        }

        Ok(())
    }

    /// Clear cache during migration
    fn clear_cache_for_migration(&self, base_dir: &Path) -> Result<()> {
        // Clear action cache directory
        let action_dir = base_dir.join("actions");
        if action_dir.exists() {
            fs::remove_dir_all(&action_dir)
                .map_err(|e| Error::file_system(&action_dir, "clear action cache", e))?;
            fs::create_dir_all(&action_dir)?;
        }

        // Clear CAS directory
        let cas_dir = base_dir.join("cas");
        if cas_dir.exists() {
            fs::remove_dir_all(&cas_dir)
                .map_err(|e| Error::file_system(&cas_dir, "clear CAS", e))?;
            fs::create_dir_all(&cas_dir)?;
        }

        log::info!("Cache cleared for migration");
        Ok(())
    }
}

impl Default for CacheMigrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_version_write() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let migrator = CacheMigrator::new();

        migrator.check_and_migrate(temp_dir.path())?;

        let version_file = temp_dir.path().join("VERSION");
        assert!(version_file.exists());

        let content = fs::read_to_string(&version_file)?;
        assert_eq!(content.trim(), CACHE_VERSION.to_string());

        Ok(())
    }

    #[test]
    fn test_version_check_same() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let version_file = temp_dir.path().join("VERSION");
        
        // Write current version
        fs::write(&version_file, CACHE_VERSION.to_string())?;

        let migrator = CacheMigrator::new();
        migrator.check_and_migrate(temp_dir.path())?;

        // Should not change anything
        let content = fs::read_to_string(&version_file)?;
        assert_eq!(content.trim(), CACHE_VERSION.to_string());

        Ok(())
    }

    #[test]
    fn test_version_too_new() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let version_file = temp_dir.path().join("VERSION");
        
        // Write newer version
        let newer_version = CACHE_VERSION + 1;
        fs::write(&version_file, newer_version.to_string())?;

        let migrator = CacheMigrator::new();
        let result = migrator.check_and_migrate(temp_dir.path());

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("newer than supported"));

        Ok(())
    }
}