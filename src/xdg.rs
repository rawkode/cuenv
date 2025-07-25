use std::env;
use std::path::PathBuf;

/// XDG Base Directory paths for cuenv
pub struct XdgPaths;

impl XdgPaths {
    /// Get XDG_CONFIG_HOME/cuenv or fallback
    pub fn config_dir() -> PathBuf {
        env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|home| home.join(".config"))
                    .unwrap_or_else(|| PathBuf::from(".config"))
            })
            .join("cuenv")
    }

    /// Get XDG_DATA_HOME/cuenv or fallback
    pub fn data_dir() -> PathBuf {
        env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|home| home.join(".local/share"))
                    .unwrap_or_else(|| PathBuf::from(".local/share"))
            })
            .join("cuenv")
    }

    /// Get XDG_STATE_HOME/cuenv or fallback
    pub fn state_dir() -> PathBuf {
        env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|home| home.join(".local/state"))
                    .unwrap_or_else(|| PathBuf::from(".local/state"))
            })
            .join("cuenv")
    }

    /// Get XDG_CACHE_HOME/cuenv or fallback
    pub fn cache_dir() -> PathBuf {
        env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|home| home.join(".cache"))
                    .unwrap_or_else(|| PathBuf::from(".cache"))
            })
            .join("cuenv")
    }

    /// Get the allowed directories file path
    pub fn allowed_file() -> PathBuf {
        Self::data_dir().join("allow")
    }

    /// Get the denied directories file path
    pub fn denied_file() -> PathBuf {
        Self::data_dir().join("deny")
    }

    /// Get the cache directory for a specific CUE file
    pub fn cache_file(cue_file: &PathBuf) -> PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        cue_file.hash(&mut hasher);
        let hash = hasher.finish();

        Self::cache_dir().join(format!("{hash:x}.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_xdg_paths_with_env() {
        env::set_var("XDG_CONFIG_HOME", "/tmp/config");
        env::set_var("XDG_DATA_HOME", "/tmp/data");
        env::set_var("XDG_STATE_HOME", "/tmp/state");
        env::set_var("XDG_CACHE_HOME", "/tmp/cache");

        assert_eq!(XdgPaths::config_dir(), PathBuf::from("/tmp/config/cuenv"));
        assert_eq!(XdgPaths::data_dir(), PathBuf::from("/tmp/data/cuenv"));
        assert_eq!(XdgPaths::state_dir(), PathBuf::from("/tmp/state/cuenv"));
        assert_eq!(XdgPaths::cache_dir(), PathBuf::from("/tmp/cache/cuenv"));

        // Clean up
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("XDG_DATA_HOME");
        env::remove_var("XDG_STATE_HOME");
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn test_specific_paths() {
        env::set_var("XDG_DATA_HOME", "/tmp/data");

        assert_eq!(
            XdgPaths::allowed_file(),
            PathBuf::from("/tmp/data/cuenv/allow")
        );
        assert_eq!(
            XdgPaths::denied_file(),
            PathBuf::from("/tmp/data/cuenv/deny")
        );

        env::remove_var("XDG_DATA_HOME");
    }
}
