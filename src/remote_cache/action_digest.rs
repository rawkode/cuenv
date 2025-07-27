//! Action digest computation for hermetic builds
//!
//! This module computes unique digests for actions based on their inputs,
//! command, environment, and platform properties. This ensures that identical
//! actions produce identical results and can be cached effectively.

use crate::remote_cache::proto::{Action, Command, Digest, EnvironmentVariable, Platform};
use crate::remote_cache::{RemoteCacheError, Result};
use sha2::{Digest as Sha2Digest, Sha256, Sha512};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Digest function to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestFunction {
    SHA256,
    SHA512,
}

impl DigestFunction {
    /// Compute digest of data
    pub fn hash(&self, data: &[u8]) -> String {
        match self {
            DigestFunction::SHA256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
            DigestFunction::SHA512 => {
                let mut hasher = Sha512::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
        }
    }
}

/// Action digest builder
pub struct ActionDigest {
    digest_function: DigestFunction,
}

impl ActionDigest {
    /// Create a new action digest builder
    pub fn new(digest_function: DigestFunction) -> Self {
        Self { digest_function }
    }

    /// Compute digest for an action
    pub async fn compute_action_digest(
        &self,
        command: &Command,
        input_root_digest: &Digest,
        platform: &Platform,
    ) -> Result<(Action, Digest)> {
        // Serialize command to compute its digest
        let command_data = self.serialize_command(command)?;
        let command_digest = self.compute_digest(&command_data);

        // Create action
        let action = Action {
            command_digest: command_digest.clone(),
            input_root_digest: input_root_digest.clone(),
            timeout: None, // Will be set by executor
            do_not_cache: false,
            platform: platform.clone(),
        };

        // Compute action digest
        let action_data = self.serialize_action(&action)?;
        let action_digest = self.compute_digest(&action_data);

        Ok((action, action_digest))
    }

    /// Compute digest for a command
    pub fn compute_command_digest(
        &self,
        arguments: Vec<String>,
        env_vars: HashMap<String, String>,
        output_files: Vec<String>,
        output_directories: Vec<String>,
        working_directory: String,
        platform: Platform,
    ) -> Result<(Command, Digest)> {
        // Sort environment variables for deterministic ordering
        let mut sorted_env: Vec<_> = env_vars.into_iter().collect();
        sorted_env.sort_by(|a, b| a.0.cmp(&b.0));

        let environment_variables = sorted_env
            .into_iter()
            .map(|(name, value)| EnvironmentVariable { name, value })
            .collect();

        let command = Command {
            arguments,
            environment_variables,
            output_files,
            output_directories,
            platform,
            working_directory,
        };

        let command_data = self.serialize_command(&command)?;
        let digest = self.compute_digest(&command_data);

        Ok((command, digest))
    }

    /// Compute digest for file content
    pub fn compute_file_digest(&self, content: &[u8]) -> Digest {
        self.compute_digest(content)
    }

    /// Compute digest for a directory tree
    pub async fn compute_directory_digest(&self, path: &Path) -> Result<Digest> {
        let tree = self.build_directory_tree(path).await?;
        let tree_data = serde_json::to_vec(&tree)?;
        Ok(self.compute_digest(&tree_data))
    }

    /// Compute digest from input files
    pub async fn compute_input_root_digest(
        &self,
        working_dir: &Path,
        input_patterns: &[String],
    ) -> Result<Digest> {
        let mut file_digests = BTreeMap::new();

        // Expand patterns and compute digests
        for pattern in input_patterns {
            let files = self.expand_pattern(working_dir, pattern)?;
            for file in files {
                let content = tokio::fs::read(&file).await?;
                let relative_path = file
                    .strip_prefix(working_dir)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .to_string();
                let digest = self.compute_file_digest(&content);
                file_digests.insert(relative_path, digest);
            }
        }

        // Create merkle tree of inputs
        let tree_data = serde_json::to_vec(&file_digests)?;
        Ok(self.compute_digest(&tree_data))
    }

    /// Build directory tree for CAS
    async fn build_directory_tree(
        &self,
        path: &Path,
    ) -> Result<crate::remote_cache::proto::Directory> {
        use crate::remote_cache::proto::{Directory, DirectoryNode, FileNode, SymlinkNode};

        let mut dir = Directory {
            files: Vec::new(),
            directories: Vec::new(),
            symlinks: Vec::new(),
        };

        let mut entries = tokio::fs::read_dir(path).await?;
        let mut items = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            items.push(entry);
        }

        // Sort for deterministic ordering
        items.sort_by_key(|e| e.file_name());

        for entry in items {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().to_string();

            if metadata.is_file() {
                let content = tokio::fs::read(entry.path()).await?;
                let digest = self.compute_file_digest(&content);
                let is_executable = Self::is_executable(&metadata);

                dir.files.push(FileNode {
                    name,
                    digest,
                    is_executable,
                });
            } else if metadata.is_dir() {
                let sub_digest = self.compute_directory_digest(&entry.path()).await?;
                dir.directories.push(DirectoryNode {
                    name,
                    digest: sub_digest,
                });
            } else if metadata.is_symlink() {
                let target = tokio::fs::read_link(entry.path())
                    .await?
                    .to_string_lossy()
                    .to_string();
                dir.symlinks.push(SymlinkNode { name, target });
            }
        }

        Ok(dir)
    }

    /// Check if file is executable
    #[cfg(unix)]
    fn is_executable(metadata: &std::fs::Metadata) -> bool {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    fn is_executable(_metadata: &std::fs::Metadata) -> bool {
        false
    }

    /// Expand glob pattern
    fn expand_pattern(&self, base: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let full_pattern = base.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let entries = glob::glob(&pattern_str)
            .map_err(|e| RemoteCacheError::Configuration(format!("Invalid glob pattern: {}", e)))?;

        let mut files = Vec::new();
        for entry in entries {
            match entry {
                Ok(path) => files.push(path),
                Err(e) => {
                    return Err(RemoteCacheError::IO(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Glob error: {}", e),
                    )))
                }
            }
        }

        Ok(files)
    }

    /// Compute digest of data
    fn compute_digest(&self, data: &[u8]) -> Digest {
        let hash = self.digest_function.hash(data);
        Digest {
            hash,
            size_bytes: data.len() as i64,
        }
    }

    /// Serialize command deterministically
    fn serialize_command(&self, command: &Command) -> Result<Vec<u8>> {
        // Use canonical JSON serialization for deterministic output
        let value = serde_json::to_value(command)?;
        canonical_json(&value)
    }

    /// Serialize action deterministically
    fn serialize_action(&self, action: &Action) -> Result<Vec<u8>> {
        let value = serde_json::to_value(action)?;
        canonical_json(&value)
    }
}

/// Canonical JSON serialization for deterministic hashing
fn canonical_json(value: &serde_json::Value) -> Result<Vec<u8>> {
    use serde_json::Value;

    fn canonical_json_impl(value: &Value, out: &mut Vec<u8>) -> Result<()> {
        match value {
            Value::Null => out.extend_from_slice(b"null"),
            Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
            Value::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
            Value::String(s) => {
                out.push(b'"');
                for c in s.chars() {
                    match c {
                        '"' => out.extend_from_slice(b"\\\""),
                        '\\' => out.extend_from_slice(b"\\\\"),
                        '\n' => out.extend_from_slice(b"\\n"),
                        '\r' => out.extend_from_slice(b"\\r"),
                        '\t' => out.extend_from_slice(b"\\t"),
                        c if c.is_control() => {
                            out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes())
                        }
                        c => {
                            let mut buf = [0; 4];
                            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        }
                    }
                }
                out.push(b'"');
            }
            Value::Array(arr) => {
                out.push(b'[');
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    canonical_json_impl(item, out)?;
                }
                out.push(b']');
            }
            Value::Object(obj) => {
                out.push(b'{');
                let mut items: Vec<_> = obj.iter().collect();
                items.sort_by_key(|(k, _)| k.as_str());
                for (i, (k, v)) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    canonical_json_impl(&Value::String(k.to_string()), out)?;
                    out.push(b':');
                    canonical_json_impl(v, out)?;
                }
                out.push(b'}');
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    canonical_json_impl(value, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digest_function() {
        let data = b"Hello, World!";

        let sha256 = DigestFunction::SHA256;
        let hash256 = sha256.hash(data);
        assert_eq!(hash256.len(), 64); // SHA256 produces 32 bytes = 64 hex chars

        let sha512 = DigestFunction::SHA512;
        let hash512 = sha512.hash(data);
        assert_eq!(hash512.len(), 128); // SHA512 produces 64 bytes = 128 hex chars
    }

    #[tokio::test]
    async fn test_action_digest() {
        let digest_builder = ActionDigest::new(DigestFunction::SHA256);

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin".to_string());
        env_vars.insert("USER".to_string(), "test".to_string());

        let (command, cmd_digest) = digest_builder
            .compute_command_digest(
                vec!["echo".to_string(), "hello".to_string()],
                env_vars,
                vec!["output.txt".to_string()],
                vec![],
                ".".to_string(),
                Platform::default(),
            )
            .unwrap();

        assert!(!cmd_digest.hash.is_empty());
        assert!(cmd_digest.size_bytes > 0);
        assert_eq!(command.arguments, vec!["echo", "hello"]);
        assert_eq!(command.environment_variables.len(), 2);
        // Environment variables should be sorted
        assert_eq!(command.environment_variables[0].name, "PATH");
        assert_eq!(command.environment_variables[1].name, "USER");
    }

    #[test]
    fn test_canonical_json() {
        let mut obj = serde_json::Map::new();
        obj.insert("z".to_string(), serde_json::json!("last"));
        obj.insert("a".to_string(), serde_json::json!("first"));
        obj.insert("m".to_string(), serde_json::json!("middle"));

        let value = serde_json::Value::Object(obj);
        let canonical = canonical_json(&value).unwrap();
        let result = String::from_utf8(canonical).unwrap();

        // Should be sorted by key
        assert_eq!(result, r#"{"a":"first","m":"middle","z":"last"}"#);
    }

    #[test]
    fn test_deterministic_hashing() {
        let digest_builder = ActionDigest::new(DigestFunction::SHA256);

        // Create two commands with same content but different order
        let mut env1 = HashMap::new();
        env1.insert("B".to_string(), "2".to_string());
        env1.insert("A".to_string(), "1".to_string());

        let mut env2 = HashMap::new();
        env2.insert("A".to_string(), "1".to_string());
        env2.insert("B".to_string(), "2".to_string());

        let (_, digest1) = digest_builder
            .compute_command_digest(
                vec!["test".to_string()],
                env1,
                vec![],
                vec![],
                ".".to_string(),
                Platform::default(),
            )
            .unwrap();

        let (_, digest2) = digest_builder
            .compute_command_digest(
                vec!["test".to_string()],
                env2,
                vec![],
                vec![],
                ".".to_string(),
                Platform::default(),
            )
            .unwrap();

        // Should produce identical digests
        assert_eq!(digest1.hash, digest2.hash);
        assert_eq!(digest1.size_bytes, digest2.size_bytes);
    }
}
