use anyhow::{Context, Result};
use dirs;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, trace};

/// Environment cache for storing evaluated hook outputs
pub struct EnvCache {
    cache_dir: PathBuf,
    cache_key: String,
    project_dir: PathBuf,
}

impl EnvCache {
    /// Create a new environment cache for the given project directory
    pub fn new(project_dir: &Path) -> Result<Self> {
        // Use dirs crate for platform-specific cache location
        let cache_base = dirs::cache_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
            .context("Could not determine cache directory")?;

        let cache_dir = cache_base.join("cuenv").join("environments");
        std::fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        // Generate cache key from project path and important files
        let cache_key = Self::generate_cache_key(project_dir)?;

        debug!(
            "Created environment cache for {} with key {}",
            project_dir.display(),
            &cache_key[..8]
        );

        Ok(EnvCache {
            cache_dir,
            cache_key,
            project_dir: project_dir.to_path_buf(),
        })
    }

    /// Generate a cache key based on project directory and key files
    fn generate_cache_key(dir: &Path) -> Result<String> {
        let mut hasher = Sha256::new();

        // Hash the canonical directory path
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        hasher.update(canonical.to_string_lossy().as_bytes());

        // Hash flake.lock if it exists (most important for nix)
        let flake_lock = dir.join("flake.lock");
        if flake_lock.exists() {
            let content = std::fs::read(&flake_lock).context("Failed to read flake.lock")?;
            hasher.update(&content);
            trace!("Included flake.lock in cache key");
        }

        // Hash env.cue if it exists
        let env_cue = dir.join("env.cue");
        if env_cue.exists() {
            let content = std::fs::read(&env_cue).context("Failed to read env.cue")?;
            hasher.update(&content);
            trace!("Included env.cue in cache key");
        }

        // Hash devenv.lock if it exists
        let devenv_lock = dir.join("devenv.lock");
        if devenv_lock.exists() {
            let content = std::fs::read(&devenv_lock).context("Failed to read devenv.lock")?;
            hasher.update(&content);
            trace!("Included devenv.lock in cache key");
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Get the path to the cache file
    pub fn cache_file(&self) -> PathBuf {
        self.cache_dir.join(format!("{}.env", self.cache_key))
    }

    /// Get the path to the profile RC file (for nix print-dev-env output)
    pub fn profile_rc(&self) -> PathBuf {
        self.cache_dir.join(format!("{}.rc", self.cache_key))
    }

    /// Load cached environment variables
    pub fn load(&self) -> Result<HashMap<String, String>> {
        let cache_file = self.cache_file();

        if !cache_file.exists() {
            debug!("Cache miss: {} does not exist", cache_file.display());
            return Err(anyhow::anyhow!("Cache file does not exist"));
        }

        debug!("Loading cached environment from {}", cache_file.display());

        // Read the cached environment file
        let content = std::fs::read_to_string(&cache_file).context("Failed to read cache file")?;

        // Parse each line as KEY=VALUE
        let mut env = HashMap::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                // Remove quotes if present
                let value = value.trim_matches('"').trim_matches('\'');
                env.insert(key.to_string(), value.to_string());
            }
        }

        info!("Loaded {} cached environment variables", env.len());
        Ok(env)
    }

    /// Save environment variables to cache
    pub fn save(&self, env: &HashMap<String, String>) -> Result<()> {
        let cache_file = self.cache_file();

        debug!("Saving {} environment variables to cache", env.len());

        // Build dotenv format content
        let mut content = String::new();
        for (key, value) in env {
            // Use shell-words to properly escape values
            let escaped = shell_words::quote(value);
            content.push_str(&format!("{key}={escaped}\n"));
        }

        // Atomic write using tempfile
        let temp = tempfile::NamedTempFile::new_in(&self.cache_dir)
            .context("Failed to create temporary cache file")?;

        std::fs::write(&temp, content).context("Failed to write to temporary cache file")?;

        temp.persist(&cache_file)
            .context("Failed to persist cache file")?;

        info!("Cached environment saved to {}", cache_file.display());
        Ok(())
    }

    /// Save raw shell RC output (for nix print-dev-env)
    pub fn save_rc(&self, rc_content: &str) -> Result<()> {
        let rc_file = self.profile_rc();

        debug!("Saving RC file to {}", rc_file.display());

        // Atomic write
        let temp = tempfile::NamedTempFile::new_in(&self.cache_dir)
            .context("Failed to create temporary RC file")?;

        std::fs::write(&temp, rc_content).context("Failed to write to temporary RC file")?;

        temp.persist(&rc_file)
            .context("Failed to persist RC file")?;

        Ok(())
    }

    /// Load raw shell RC output
    pub fn load_rc(&self) -> Result<String> {
        let rc_file = self.profile_rc();

        if !rc_file.exists() {
            return Err(anyhow::anyhow!("RC file does not exist"));
        }

        std::fs::read_to_string(&rc_file).context("Failed to read RC file")
    }

    /// Clear this specific cache
    pub fn clear(&self) -> Result<()> {
        let cache_file = self.cache_file();
        let rc_file = self.profile_rc();

        if cache_file.exists() {
            std::fs::remove_file(&cache_file).context("Failed to remove cache file")?;
        }

        if rc_file.exists() {
            std::fs::remove_file(&rc_file).context("Failed to remove RC file")?;
        }

        info!("Cleared cache for {}", self.project_dir.display());
        Ok(())
    }

    /// Clear all caches
    pub fn clear_all() -> Result<()> {
        let cache_base = dirs::cache_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
            .context("Could not determine cache directory")?;

        let cache_dir = cache_base.join("cuenv").join("environments");

        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir).context("Failed to remove cache directory")?;
            info!("Cleared all environment caches");
        }

        Ok(())
    }

    /// Get information about cached environments
    pub fn list_caches() -> Result<Vec<PathBuf>> {
        let cache_base = dirs::cache_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
            .context("Could not determine cache directory")?;

        let cache_dir = cache_base.join("cuenv").join("environments");

        if !cache_dir.exists() {
            return Ok(Vec::new());
        }

        let mut caches = Vec::new();
        for entry in std::fs::read_dir(&cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("env") {
                caches.push(path);
            }
        }

        Ok(caches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let cache = EnvCache::new(temp_dir.path()).unwrap();

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test value".to_string());
        env.insert("PATH".to_string(), "/usr/bin:/usr/local/bin".to_string());

        // Save environment
        cache.save(&env).unwrap();

        // Load it back
        let loaded = cache.load().unwrap();

        assert_eq!(loaded.get("TEST_VAR"), Some(&"test value".to_string()));
        assert_eq!(
            loaded.get("PATH"),
            Some(&"/usr/bin:/usr/local/bin".to_string())
        );
    }

    #[test]
    fn test_cache_key_generation() {
        let temp_dir = TempDir::new().unwrap();

        // Create some files
        std::fs::write(temp_dir.path().join("env.cue"), "test content").unwrap();

        let cache1 = EnvCache::new(temp_dir.path()).unwrap();
        let key1 = cache1.cache_key.clone();

        // Modify the file
        std::fs::write(temp_dir.path().join("env.cue"), "different content").unwrap();

        let cache2 = EnvCache::new(temp_dir.path()).unwrap();
        let key2 = cache2.cache_key.clone();

        // Keys should be different
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_rc_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let cache = EnvCache::new(temp_dir.path()).unwrap();

        let rc_content = r#"
export PATH="/nix/store/abc/bin:$PATH"
export CARGO_HOME="/home/user/.cargo"
"#;

        // Save RC content
        cache.save_rc(rc_content).unwrap();

        // Load it back
        let loaded = cache.load_rc().unwrap();
        assert_eq!(loaded, rc_content);
    }
}
