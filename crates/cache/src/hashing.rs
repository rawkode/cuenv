use cuenv_core::{Error, Result};
use cuenv_utils::atomic_file::write_atomic_string;
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Content hasher for generating cache keys
#[derive(Debug)]
pub struct ContentHasher {
    /// Label for debugging purposes
    pub label: String,
    hasher: Sha256,
    /// Metadata about what was hashed
    pub manifest: HashManifest,
}

/// Manifest containing metadata about what was hashed
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HashManifest {
    pub label: String,
    pub inputs: Vec<String>,
    pub files: HashMap<String, String>,
}

impl ContentHasher {
    /// Create a new content hasher with a label
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            hasher: Sha256::new(),
            manifest: HashManifest {
                label: label.to_string(),
                inputs: Vec::new(),
                files: HashMap::new(),
            },
        }
    }

    /// Hash arbitrary content
    pub fn hash_content<T: Serialize>(&mut self, content: T) -> Result<()> {
        let serialized = serde_json::to_string(&content).map_err(|e| Error::Json {
            message: "Failed to serialize content for hashing".to_string(),
            source: e,
        })?;

        self.hasher.update(serialized.as_bytes());
        self.manifest
            .inputs
            .push(format!("content:{}", serialized.len()));

        Ok(())
    }

    /// Hash a file's content using streaming to handle large files efficiently
    pub fn hash_file(&mut self, file_path: &Path) -> Result<()> {
        use std::io::{BufReader, Read};

        // Use O_NOFOLLOW on Unix to prevent symlink attacks
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;

            // Open file with O_NOFOLLOW to prevent symlink TOCTOU attacks
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(file_path)
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        Error::configuration(format!(
                            "Symlink detected or permission denied: {file_path:?}"
                        ))
                    } else {
                        Error::file_system(file_path.to_path_buf(), "open file for hashing", e)
                    }
                })?;

            let mut reader = BufReader::with_capacity(8192, file);
            let mut file_hasher = Sha256::new();
            let mut buffer = [0u8; 8192];

            // Stream the file in chunks
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| {
                    Error::file_system(file_path.to_path_buf(), "read file chunk for hashing", e)
                })?;

                if bytes_read == 0 {
                    break;
                }

                let chunk = &buffer[..bytes_read];
                file_hasher.update(chunk);
                self.hasher.update(chunk);
            }

            let file_hash = format!("{:x}", file_hasher.finalize());
            let path_str = file_path.to_string_lossy().to_string();
            self.manifest.files.insert(path_str.clone(), file_hash);
            self.manifest.inputs.push(format!("file:{path_str}"));

            Ok(())
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, use lstat to check for symlinks before opening
            let metadata = file_path.symlink_metadata().map_err(|e| {
                Error::file_system(file_path.to_path_buf(), "get symlink metadata", e)
            })?;

            if metadata.file_type().is_symlink() {
                return Err(Error::configuration(format!(
                    "Symlinks are not allowed for security reasons: {:?}",
                    file_path
                )));
            }

            // Open file for streaming
            let file = fs::File::open(file_path).map_err(|e| {
                Error::file_system(file_path.to_path_buf(), "open file for hashing", e)
            })?;

            let mut reader = BufReader::with_capacity(8192, file);
            let mut file_hasher = Sha256::new();
            let mut buffer = [0u8; 8192];

            // Stream the file in chunks
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| {
                    Error::file_system(file_path.to_path_buf(), "read file chunk for hashing", e)
                })?;

                if bytes_read == 0 {
                    break;
                }

                let chunk = &buffer[..bytes_read];
                file_hasher.update(chunk);
                self.hasher.update(chunk);
            }

            let file_hash = format!("{:x}", file_hasher.finalize());
            let path_str = file_path.to_string_lossy().to_string();
            self.manifest.files.insert(path_str.clone(), file_hash);
            self.manifest.inputs.push(format!("file:{path_str}"));

            Ok(())
        }
    }

    /// Hash files matching a glob pattern
    pub fn hash_glob(&mut self, pattern: &str, base_dir: &Path) -> Result<()> {
        let files = expand_glob_pattern(pattern, base_dir)?;

        // Sort files for consistent hashing
        let mut sorted_files = files;
        sorted_files.sort();

        for file in sorted_files {
            self.hash_file(&file)?;
        }

        Ok(())
    }

    /// Generate the final hash
    pub fn generate_hash(&mut self) -> Result<String> {
        let result = self.hasher.finalize_reset();
        Ok(format!("{result:x}"))
    }

    /// Serialize the manifest for storage
    pub fn serialize(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.manifest).map_err(|e| Error::Json {
            message: "Failed to serialize hash manifest".to_string(),
            source: e,
        })
    }
}

/// Hash engine for managing cache hash manifests
#[derive(Debug)]
pub struct HashEngine {
    /// Directory for storing hash manifests
    pub hashes_dir: PathBuf,
}

impl HashEngine {
    /// Create a new hash engine
    pub fn new(cache_dir: &Path) -> Result<HashEngine> {
        let hashes_dir = cache_dir.join("hashes");

        log::debug!("Creating hash engine with hashes_dir: {hashes_dir:?}");

        fs::create_dir_all(&hashes_dir)
            .map_err(|e| Error::file_system(hashes_dir.clone(), "create hashes directory", e))?;

        Ok(HashEngine { hashes_dir })
    }

    /// Create a new content hasher
    pub fn create_hasher(&self, label: &str) -> ContentHasher {
        ContentHasher::new(label)
    }

    /// Get the path for a hash manifest
    pub fn get_manifest_path(&self, hash: &str) -> PathBuf {
        self.hashes_dir.join(format!("{hash}.json"))
    }

    /// Save a hash manifest to disk
    pub fn save_manifest(&self, hasher: &ContentHasher, hash: &str) -> Result<()> {
        let path = self.get_manifest_path(hash);

        log::debug!("Saving hash manifest for '{}' to {:?}", hasher.label, path);

        let data = hasher.serialize()?;
        write_atomic_string(&path, &data)?;

        Ok(())
    }
}

/// Expand a glob pattern to find matching files
pub fn expand_glob_pattern(pattern: &str, base_dir: &Path) -> Result<Vec<PathBuf>> {
    // Check if it's a direct file path (no glob chars)
    if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
        let full_path = base_dir.join(pattern);
        // Use metadata to avoid TOCTOU between exists() and is_file()/is_dir()
        match full_path.metadata() {
            Ok(metadata) => {
                if metadata.is_file() {
                    return Ok(vec![full_path]);
                } else if metadata.is_dir() {
                    // If it's a directory, include all files recursively
                    let mut files = Vec::new();
                    collect_files_recursive(&full_path, &mut files)?;
                    return Ok(files);
                }
            }
            Err(_) => {
                // Path doesn't exist or can't be accessed
                return Ok(Vec::new());
            }
        }
    }

    // Build glob pattern
    let glob = Glob::new(pattern)
        .map_err(|e| Error::configuration(format!("Invalid glob pattern '{pattern}': {e}")))?;

    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    let globset = builder
        .build()
        .map_err(|e| Error::configuration(format!("Failed to build globset: {e}")))?;

    // Walk directory and find matches
    let mut files = Vec::new();
    walk_directory_for_glob(base_dir, base_dir, &globset, &mut files)?;

    Ok(files)
}

/// Recursively walk directory and collect files matching the globset
fn walk_directory_for_glob(
    dir: &Path,
    base_dir: &Path,
    globset: &globset::GlobSet,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    // Open directory handle first to prevent TOCTOU

    // Canonicalize base_dir once at the start
    let canonical_base = base_dir.canonicalize().map_err(|e| {
        Error::file_system(base_dir.to_path_buf(), "canonicalize base directory", e)
    })?;

    // Open and validate directory using file descriptor
    let dir_handle = fs::File::open(dir)
        .map_err(|e| Error::file_system(dir.to_path_buf(), "open directory", e))?;

    let dir_metadata = dir_handle.metadata().map_err(|e| {
        Error::file_system(dir.to_path_buf(), "get directory metadata from handle", e)
    })?;

    if !dir_metadata.is_dir() {
        return Err(Error::configuration(format!(
            "Path is not a directory: {dir:?}"
        )));
    }

    // Get canonical path from the file descriptor to prevent TOCTOU
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        // Get path from file descriptor
        let fd = dir_handle.as_raw_fd();
        let fd_path = PathBuf::from(format!("/proc/self/fd/{fd}"));

        if let Ok(canonical_dir) = fd_path.read_link() {
            if !canonical_dir.starts_with(&canonical_base) {
                return Err(Error::configuration(format!(
                    "Directory traversal detected: {dir:?} is outside of base directory {base_dir:?}"
                )));
            }
        }
    }

    // On non-Unix, fall back to canonicalize with the understanding of potential TOCTOU
    #[cfg(not(unix))]
    {
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| Error::file_system(dir.to_path_buf(), "canonicalize directory", e))?;

        if !canonical_dir.starts_with(&canonical_base) {
            return Err(Error::configuration(format!(
                "Directory traversal detected: {dir:?} is outside of base directory {base_dir:?}"
            )));
        }
    }

    let entries = fs::read_dir(dir)
        .map_err(|e| Error::file_system(dir.to_path_buf(), "read directory", e))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::file_system(dir.to_path_buf(), "read directory entry", e))?;
        let path = entry.path();

        if path.is_file() {
            // Get relative path for matching
            let relative = path.strip_prefix(base_dir).unwrap_or(&path);
            if globset.is_match(relative) {
                files.push(path);
            }
        } else if path.is_dir() {
            // Use file_type() from DirEntry to avoid TOCTOU
            // This is atomic with the directory read operation
            let file_type = entry
                .file_type()
                .map_err(|e| Error::file_system(path.clone(), "get file type", e))?;

            // Skip symlinks to prevent traversal
            if !file_type.is_symlink() {
                walk_directory_for_glob(&path, base_dir, globset, files)?;
            }
        }
    }

    Ok(())
}

/// Recursively collect all files in a directory
fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    // Validate directory before reading to prevent TOCTOU
    let dir_metadata = dir
        .metadata()
        .map_err(|e| Error::file_system(dir.to_path_buf(), "get directory metadata", e))?;

    if !dir_metadata.is_dir() {
        return Err(Error::configuration(format!(
            "Path is not a directory: {dir:?}"
        )));
    }

    let entries = fs::read_dir(dir)
        .map_err(|e| Error::file_system(dir.to_path_buf(), "read directory", e))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::file_system(dir.to_path_buf(), "read directory entry", e))?;
        let path = entry.path();

        // Use entry metadata instead of path metadata to avoid TOCTOU
        let metadata = entry
            .metadata()
            .map_err(|e| Error::file_system(path.clone(), "get metadata", e))?;

        if metadata.is_file() {
            // Re-validate that it's still a file
            match path.metadata() {
                Ok(m) if m.is_file() => files.push(path),
                _ => {
                    // File was removed or changed - skip it
                    log::debug!("File disappeared or changed during collection: {path:?}");
                }
            }
        } else if metadata.is_dir() {
            // Re-stat using lstat equivalent to check for symlinks atomically
            #[cfg(unix)]
            {
                // If it's a directory according to stat but also a symlink,
                // that means it's a symlink to a directory, which we skip
                let symlink_metadata = path
                    .symlink_metadata()
                    .map_err(|e| Error::file_system(path.clone(), "get symlink metadata", e))?;
                if !symlink_metadata.file_type().is_symlink() {
                    collect_files_recursive(&path, files)?;
                }
            }
            #[cfg(not(unix))]
            {
                // On non-Unix, fall back to checking file type
                if !metadata.file_type().is_symlink() {
                    collect_files_recursive(&path, files)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_content_hasher_creation() {
        let hasher = ContentHasher::new("test_hasher");
        assert_eq!(hasher.label, "test_hasher");
        assert_eq!(hasher.manifest.label, "test_hasher");
        assert!(hasher.manifest.inputs.is_empty());
        assert!(hasher.manifest.files.is_empty());
    }

    #[test]
    fn test_hash_consistency() {
        let mut hasher1 = ContentHasher::new("test");
        let mut hasher2 = ContentHasher::new("test");

        let test_data = "consistent test data";
        hasher1
            .hash_content(&test_data)
            .expect("Failed to hash content");
        hasher2
            .hash_content(&test_data)
            .expect("Failed to hash content");

        let hash1 = hasher1.finalize();
        let hash2 = hasher2.finalize();

        assert_eq!(hash1, hash2, "Same input should produce same hash");
    }

    #[test]
    fn test_hash_different_inputs() {
        let mut hasher1 = ContentHasher::new("test");
        let mut hasher2 = ContentHasher::new("test");

        hasher1
            .hash_content("input1")
            .expect("Failed to hash content");
        hasher2
            .hash_content("input2")
            .expect("Failed to hash content");

        let hash1 = hasher1.finalize();
        let hash2 = hasher2.finalize();

        assert_ne!(
            hash1, hash2,
            "Different inputs should produce different hashes"
        );
    }

    #[test]
    fn test_hash_file_content() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test_file.txt");
        let test_content = "Hello, World!";

        fs::write(&file_path, test_content).expect("Failed to write test file");

        let mut hasher = ContentHasher::new("file_test");
        hasher.hash_file(&file_path).expect("Failed to hash file");

        let hash = hasher.finalize();
        assert!(!hash.is_empty(), "Hash should not be empty");

        // Verify the file was recorded in manifest
        let file_key = file_path.to_string_lossy();
        assert!(hasher.manifest.files.contains_key(file_key.as_ref()));
    }

    #[test]
    fn test_hash_file_nonexistent() {
        let mut hasher = ContentHasher::new("test");
        let nonexistent_path = Path::new("/nonexistent/file.txt");

        let result = hasher.hash_file(nonexistent_path);
        assert!(result.is_err(), "Should fail when hashing nonexistent file");
    }

    #[test]
    fn test_hash_glob_patterns() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create test files
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        fs::write(temp_dir.path().join("other.log"), "log content").unwrap();

        let mut hasher = ContentHasher::new("glob_test");
        let patterns = vec!["*.txt".to_string()];

        hasher
            .hash_glob(temp_dir.path(), &patterns)
            .expect("Failed to hash glob");

        // Should have hashed the .txt files but not the .log file
        assert!(
            hasher.manifest.files.len() >= 2,
            "Should have hashed at least 2 files"
        );

        // Check that .txt files are in manifest
        let has_txt_files = hasher.manifest.files.keys().any(|k| k.ends_with(".txt"));
        assert!(has_txt_files, "Should have .txt files in manifest");
    }

    #[test]
    fn test_empty_directory_hash() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let empty_subdir = temp_dir.path().join("empty");
        fs::create_dir(&empty_subdir).unwrap();

        let mut hasher = ContentHasher::new("empty_test");
        let patterns = vec!["*".to_string()];

        // Should not fail on empty directory
        hasher
            .hash_glob(&empty_subdir, &patterns)
            .expect("Should handle empty directory");

        // Manifest should be empty for no files
        assert!(hasher.manifest.files.is_empty());
    }

    #[test]
    fn test_collect_files_recursive_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create nested structure
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(temp_dir.path().join("root.txt"), "root").unwrap();
        fs::write(subdir.join("nested.txt"), "nested").unwrap();

        let mut files = Vec::new();
        collect_files_recursive(temp_dir.path(), &mut files).expect("Failed to collect files");

        assert_eq!(files.len(), 2, "Should find 2 files");

        let has_root = files.iter().any(|p| p.file_name().unwrap() == "root.txt");
        let has_nested = files.iter().any(|p| p.file_name().unwrap() == "nested.txt");
        assert!(
            has_root && has_nested,
            "Should find both root and nested files"
        );
    }

    #[test]
    fn test_hash_serialization_order() {
        // Test that hash order is deterministic for different insertion orders
        let mut hasher1 = ContentHasher::new("order_test");
        let mut hasher2 = ContentHasher::new("order_test");

        // Add same content in different order
        hasher1.hash_content("A").unwrap();
        hasher1.hash_content("B").unwrap();

        hasher2.hash_content("B").unwrap();
        hasher2.hash_content("A").unwrap();

        let hash1 = hasher1.finalize();
        let hash2 = hasher2.finalize();

        assert_ne!(hash1, hash2, "Hash should depend on insertion order");
    }
}
