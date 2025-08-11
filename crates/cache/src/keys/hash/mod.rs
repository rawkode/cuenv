//! Hash computation and path normalization for cache keys

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// Compute hash for cache key generation
pub struct HashComputer;

impl HashComputer {
    /// Generate a cache key hash from various inputs
    pub fn compute_hash(
        task_name: &str,
        task_config_hash: &str,
        working_dir: &str,
        input_files: &HashMap<String, String>,
        env_vars: &HashMap<String, String>,
        command: Option<&str>,
    ) -> String {
        let mut hasher = Sha256::new();

        // Include task name
        hasher.update(task_name.as_bytes());

        // Include task configuration hash
        hasher.update(task_config_hash.as_bytes());

        // Include working directory
        hasher.update(working_dir.as_bytes());

        // Include command/script if present
        if let Some(cmd) = command {
            hasher.update(cmd.as_bytes());
        }

        // Include input file hashes
        let mut sorted_files: Vec<_> = input_files.iter().collect();
        sorted_files.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (path, hash) in sorted_files {
            hasher.update(path.as_bytes());
            hasher.update(hash.as_bytes());
        }

        // Include environment variables
        let mut sorted_env: Vec<_> = env_vars.iter().collect();
        sorted_env.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (key, value) in sorted_env {
            hasher.update(key.as_bytes());
            hasher.update(value.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Normalize working directory path for consistent cache keys across platforms
    pub fn normalize_working_dir(path: &Path) -> String {
        // Normalize path separators to forward slashes for consistency
        let path_str = path.to_string_lossy();
        let mut normalized = path_str.replace('\\', "/");

        // Remove trailing slashes and dots for consistency
        while normalized.ends_with('/') || normalized.ends_with("/.") {
            if normalized.ends_with("/.") {
                normalized.truncate(normalized.len() - 2);
            } else if normalized.ends_with('/') {
                normalized.truncate(normalized.len() - 1);
            }
        }

        // Handle path components like `/tmp/../project` by resolving them
        // This is a simplified path resolution that doesn't access the filesystem
        let mut components = Vec::new();
        for component in normalized.split('/') {
            match component {
                "" | "." => continue, // Skip empty and current directory references
                ".." => {
                    if !components.is_empty() && components.last() != Some(&"..") {
                        components.pop(); // Go up one directory
                    } else if !normalized.starts_with('/') {
                        // For relative paths, keep the ".."
                        components.push(component);
                    }
                    // For absolute paths, ".." at root is ignored
                }
                _ => components.push(component),
            }
        }

        let resolved = if normalized.starts_with('/') {
            format!("/{}", components.join("/"))
        } else {
            components.join("/")
        };

        // Handle empty path case
        if resolved.is_empty() || resolved == "/" {
            return "/".to_string();
        }

        // Convert relative paths to absolute-style paths for consistency
        if !resolved.starts_with('/') && !resolved.contains(':') {
            format!("/{resolved}")
        } else if cfg!(windows) && resolved.len() > 1 && resolved.chars().nth(1) == Some(':') {
            // Convert Windows drive letters to forward-slash format (C: -> /c)
            let drive_letter = resolved.chars().next().unwrap().to_lowercase();
            let rest = &resolved[2..];
            format!("/{drive_letter}{rest}")
        } else {
            resolved
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let mut input_files = HashMap::new();
        input_files.insert("src/main.rs".to_string(), "hash1".to_string());

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin".to_string());
        env_vars.insert("HOME".to_string(), "/home/user".to_string());

        let hash1 = HashComputer::compute_hash(
            "build",
            "config_hash",
            "/project",
            &input_files,
            &env_vars,
            Some("cargo build"),
        );

        let hash2 = HashComputer::compute_hash(
            "build",
            "config_hash",
            "/project",
            &input_files,
            &env_vars,
            Some("cargo build"),
        );

        // Same inputs should produce same hash
        assert_eq!(hash1, hash2);

        // Different command should produce different hash
        let hash3 = HashComputer::compute_hash(
            "build",
            "config_hash",
            "/project",
            &input_files,
            &env_vars,
            Some("cargo test"),
        );

        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_normalize_working_dir() {
        assert_eq!(
            HashComputer::normalize_working_dir(Path::new("/project")),
            "/project"
        );
        assert_eq!(
            HashComputer::normalize_working_dir(Path::new("/project/")),
            "/project"
        );
        assert_eq!(
            HashComputer::normalize_working_dir(Path::new("/project/.")),
            "/project"
        );
        assert_eq!(
            HashComputer::normalize_working_dir(Path::new("/tmp/../project")),
            "/project"
        );
        assert_eq!(HashComputer::normalize_working_dir(Path::new("/")), "/");
    }
}
