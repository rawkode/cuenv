//! Per-directory locking mechanism for hook execution

use crate::paths::{ensure_state_dir_exists, get_supervisor_lock_path};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// A lock for a specific directory to prevent concurrent hook execution
#[derive(Debug)]
pub struct DirectoryLock {
    lock_file: File,
    lock_path: PathBuf,
    directory: PathBuf,
    pid: u32,
}

impl DirectoryLock {
    /// Try to acquire a lock for the given directory
    pub fn try_acquire(directory: &Path) -> io::Result<Self> {
        ensure_state_dir_exists(directory)?;
        let lock_path = get_supervisor_lock_path(directory);

        // Try to open/create the lock file
        let mut lock_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        // Try to acquire an exclusive lock (non-blocking)
        match lock_file.try_lock_exclusive() {
            Ok(_) => {
                // We got the lock! Write our PID to the file
                let pid = std::process::id();
                lock_file.set_len(0)?; // Truncate the file
                writeln!(lock_file, "{pid}")?;
                lock_file.sync_all()?;

                Ok(Self {
                    lock_file,
                    lock_path,
                    directory: directory.to_path_buf(),
                    pid,
                })
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Lock is held by another process
                // Check if that process is still alive
                if let Ok(contents) = fs::read_to_string(&lock_path) {
                    if let Ok(lock_pid) = contents.trim().parse::<u32>() {
                        if !is_process_running(lock_pid) {
                            // Process is dead, steal the lock
                            drop(lock_file); // Close the file first
                            fs::remove_file(&lock_path)?;
                            // Recursively try again
                            return Self::try_acquire(directory);
                        }
                    }
                }

                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "Hooks already running for directory: {}",
                        directory.display()
                    ),
                ))
            }
            Err(e) => Err(e),
        }
    }

    /// Get the PID that owns this lock
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the directory this lock is for
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl Drop for DirectoryLock {
    fn drop(&mut self) {
        // Unlock the file
        let _ = fs2::FileExt::unlock(&self.lock_file);
        // Remove the lock file
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Check if a process with the given PID is running
fn is_process_running(pid: u32) -> bool {
    // If it's the current process, it's definitely running
    if pid == std::process::id() {
        return true;
    }

    #[cfg(unix)]
    {
        // Try using kill with signal 0 to check if process exists
        // This is more reliable than checking /proc which might not be available in sandboxes
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    #[cfg(not(unix))]
    {
        // For non-Unix systems, conservatively assume it's running
        // This prevents accidental lock stealing
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_directory_lock_exclusive() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // First lock should succeed
        let lock1 = DirectoryLock::try_acquire(dir_path).unwrap();

        // Second lock should fail
        let lock2 = DirectoryLock::try_acquire(dir_path);
        assert!(lock2.is_err());
        assert_eq!(lock2.unwrap_err().kind(), io::ErrorKind::WouldBlock);

        // Drop first lock
        drop(lock1);

        // Now we should be able to acquire the lock again
        let lock3 = DirectoryLock::try_acquire(dir_path).unwrap();
        drop(lock3);
    }
}
