use crate::cache::{CacheError, CacheResult};
use std::path::Path;
use tracing::{debug, info, warn};

/// Platform-specific optimizations for cache performance
pub struct PlatformOptimizations;

impl PlatformOptimizations {
    /// Apply all platform-specific optimizations
    pub fn apply_all(cache_path: &Path) -> CacheResult<()> {
        info!("Applying platform-specific optimizations");

        #[cfg(target_os = "linux")]
        Self::apply_linux_optimizations(cache_path)?;

        #[cfg(target_os = "macos")]
        Self::apply_macos_optimizations(cache_path)?;

        #[cfg(target_os = "windows")]
        Self::apply_windows_optimizations(cache_path)?;

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn apply_linux_optimizations(cache_path: &Path) -> CacheResult<()> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        info!("Applying Linux-specific optimizations");

        // 1. Set optimal file permissions (rwx for user only)
        if let Ok(metadata) = fs::metadata(cache_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(cache_path, perms).ok();
        }

        // 2. Enable transparent huge pages for cache directory
        Self::enable_transparent_huge_pages()?;

        // 3. Set I/O scheduler to deadline for SSDs
        Self::optimize_io_scheduler()?;

        // 4. Increase file descriptor limits
        Self::increase_fd_limits()?;

        // 5. Enable memory-mapped file optimizations
        Self::optimize_mmap_settings()?;

        // 6. Set CPU affinity for cache threads
        Self::set_cpu_affinity()?;

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn enable_transparent_huge_pages() -> CacheResult<()> {
        use std::fs;

        let thp_path = "/sys/kernel/mm/transparent_hugepage/enabled";
        match fs::write(thp_path, b"madvise") {
            Ok(_) => {
                debug!("Enabled transparent huge pages in madvise mode");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to enable transparent huge pages: {}", e);
                Ok(()) // Non-fatal
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn optimize_io_scheduler() -> CacheResult<()> {
        use std::fs;
        use std::path::PathBuf;

        // Find block devices
        let block_path = PathBuf::from("/sys/block");
        if let Ok(entries) = fs::read_dir(&block_path) {
            for entry in entries.flatten() {
                let scheduler_path = entry.path().join("queue/scheduler");
                if scheduler_path.exists() {
                    // Try to set deadline scheduler for better SSD performance
                    fs::write(&scheduler_path, b"deadline").ok();
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn increase_fd_limits() -> CacheResult<()> {
        use libc::{rlimit, setrlimit, RLIMIT_NOFILE};

        unsafe {
            let mut rlim = rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };

            if libc::getrlimit(RLIMIT_NOFILE, &mut rlim) == 0 {
                // Try to set to a high value
                rlim.rlim_cur = rlim.rlim_cur.max(65536);
                if rlim.rlim_max < rlim.rlim_cur {
                    rlim.rlim_max = rlim.rlim_cur;
                }

                if setrlimit(RLIMIT_NOFILE, &rlim) == 0 {
                    debug!("Increased file descriptor limit to {}", rlim.rlim_cur);
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn optimize_mmap_settings() -> CacheResult<()> {
        use std::fs;

        // Increase max_map_count for better mmap performance
        let max_map_count_path = "/proc/sys/vm/max_map_count";
        fs::write(max_map_count_path, b"262144").ok();

        // Optimize dirty page writeback
        fs::write("/proc/sys/vm/dirty_ratio", b"80").ok();
        fs::write("/proc/sys/vm/dirty_background_ratio", b"5").ok();

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn set_cpu_affinity() -> CacheResult<()> {
        use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
        use std::mem;

        unsafe {
            let mut cpu_set: cpu_set_t = mem::zeroed();
            CPU_ZERO(&mut cpu_set);

            // Use all available CPUs
            let num_cpus = libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as usize;
            for i in 0..num_cpus {
                CPU_SET(i, &mut cpu_set);
            }

            if sched_setaffinity(0, mem::size_of::<cpu_set_t>(), &cpu_set) == 0 {
                debug!("Set CPU affinity to use all {} CPUs", num_cpus);
            }
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn apply_macos_optimizations(cache_path: &Path) -> CacheResult<()> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        info!("Applying macOS-specific optimizations");

        // 1. Set optimal file permissions
        if let Ok(metadata) = fs::metadata(cache_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(cache_path, perms).ok();
        }

        // 2. Disable Spotlight indexing for cache directory
        Self::disable_spotlight_indexing(cache_path)?;

        // 3. Enable APFS optimizations
        Self::enable_apfs_optimizations(cache_path)?;

        // 4. Increase file descriptor limits
        Self::increase_macos_fd_limits()?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn disable_spotlight_indexing(path: &Path) -> CacheResult<()> {
        use std::process::Command;

        let output = Command::new("mdutil")
            .args(&["-i", "off", path.to_str().unwrap_or(".")])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                debug!("Disabled Spotlight indexing for cache directory");
                Ok(())
            }
            _ => {
                warn!("Failed to disable Spotlight indexing");
                Ok(()) // Non-fatal
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn enable_apfs_optimizations(path: &Path) -> CacheResult<()> {
        use std::fs;

        // Create .nobackup file to exclude from Time Machine
        let nobackup_path = path.join(".nobackup");
        fs::write(nobackup_path, b"").ok();

        // Set extended attributes for better performance
        Self::set_macos_xattrs(path)?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn set_macos_xattrs(path: &Path) -> CacheResult<()> {
        use std::ffi::CString;
        use std::os::raw::c_char;

        extern "C" {
            fn setxattr(
                path: *const c_char,
                name: *const c_char,
                value: *const u8,
                size: usize,
                position: u32,
                options: i32,
            ) -> i32;
        }

        unsafe {
            let path_c = CString::new(path.to_str().unwrap_or(".")).unwrap();

            // Disable backup
            let backup_attr =
                CString::new("com.apple.metadata:com_apple_backup_excludeItem").unwrap();
            let backup_value = b"com.apple.backupd";
            setxattr(
                path_c.as_ptr(),
                backup_attr.as_ptr(),
                backup_value.as_ptr(),
                backup_value.len(),
                0,
                0,
            );

            // Mark as cache directory
            let cache_attr = CString::new("com.apple.system.Security").unwrap();
            let cache_value = b"cache";
            setxattr(
                path_c.as_ptr(),
                cache_attr.as_ptr(),
                cache_value.as_ptr(),
                cache_value.len(),
                0,
                0,
            );
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn increase_macos_fd_limits() -> CacheResult<()> {
        use libc::{rlimit, setrlimit, RLIMIT_NOFILE};

        unsafe {
            let mut rlim = rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };

            if libc::getrlimit(RLIMIT_NOFILE, &mut rlim) == 0 {
                rlim.rlim_cur = rlim.rlim_cur.max(10240);
                if rlim.rlim_max < rlim.rlim_cur {
                    rlim.rlim_max = rlim.rlim_cur;
                }

                if setrlimit(RLIMIT_NOFILE, &rlim) == 0 {
                    debug!("Increased file descriptor limit to {}", rlim.rlim_cur);
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn apply_windows_optimizations(cache_path: &Path) -> CacheResult<()> {
        use std::ffi::OsStr;
        use std::fs;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::fileapi::{SetFileAttributesW, FILE_ATTRIBUTE_NOT_CONTENT_INDEXED};

        info!("Applying Windows-specific optimizations");

        // 1. Disable content indexing
        let wide_path: Vec<u16> = OsStr::new(cache_path.to_str().unwrap_or("."))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            SetFileAttributesW(wide_path.as_ptr(), FILE_ATTRIBUTE_NOT_CONTENT_INDEXED);
        }

        // 2. Enable file system compression for better disk usage
        Self::enable_ntfs_compression(cache_path)?;

        // 3. Optimize Windows Defender exclusions
        Self::add_defender_exclusion(cache_path)?;

        // 4. Set process priority
        Self::set_windows_process_priority()?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn enable_ntfs_compression(path: &Path) -> CacheResult<()> {
        use std::process::Command;

        let output = Command::new("compact")
            .args(&["/c", "/s", path.to_str().unwrap_or(".")])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                debug!("Enabled NTFS compression for cache directory");
                Ok(())
            }
            _ => {
                warn!("Failed to enable NTFS compression");
                Ok(()) // Non-fatal
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn add_defender_exclusion(path: &Path) -> CacheResult<()> {
        use std::process::Command;

        let output = Command::new("powershell")
            .args(&[
                "-Command",
                &format!(
                    "Add-MpPreference -ExclusionPath '{}'",
                    path.to_str().unwrap_or(".")
                ),
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                debug!("Added Windows Defender exclusion for cache directory");
                Ok(())
            }
            _ => {
                warn!("Failed to add Windows Defender exclusion (requires admin rights)");
                Ok(()) // Non-fatal
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn set_windows_process_priority() -> CacheResult<()> {
        use winapi::um::processthreadsapi::{GetCurrentProcess, SetPriorityClass};
        use winapi::um::winbase::HIGH_PRIORITY_CLASS;

        unsafe {
            let process = GetCurrentProcess();
            if SetPriorityClass(process, HIGH_PRIORITY_CLASS) != 0 {
                debug!("Set process priority to HIGH");
            }
        }

        Ok(())
    }

    /// Get platform-specific cache directory recommendations
    pub fn recommended_cache_path() -> std::path::PathBuf {
        use std::path::PathBuf;

        #[cfg(target_os = "linux")]
        {
            // Use XDG cache directory
            if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
                return PathBuf::from(xdg_cache).join("cuenv");
            }

            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(".cache/cuenv");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join("Library/Caches/cuenv");
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                return PathBuf::from(local_app_data).join("cuenv\\cache");
            }
        }

        // Fallback
        PathBuf::from(".cache")
    }

    /// Get optimal cache configuration for the platform
    pub fn optimal_cache_config() -> OptimalCacheConfig {
        #[cfg(target_os = "linux")]
        {
            OptimalCacheConfig {
                shard_count: 512,     // More shards for better parallelism
                compression_level: 3, // Moderate compression
                use_mmap: true,
                max_file_size: 100 * 1024 * 1024, // 100MB
                sync_writes: false,               // Rely on OS buffering
            }
        }

        #[cfg(target_os = "macos")]
        {
            OptimalCacheConfig {
                shard_count: 256,
                compression_level: 2, // Lower compression for APFS
                use_mmap: true,
                max_file_size: 50 * 1024 * 1024, // 50MB
                sync_writes: true,               // More aggressive syncing on macOS
            }
        }

        #[cfg(target_os = "windows")]
        {
            OptimalCacheConfig {
                shard_count: 128,                // Fewer shards on Windows
                compression_level: 5,            // Higher compression to compensate for NTFS
                use_mmap: false,                 // mmap less efficient on Windows
                max_file_size: 25 * 1024 * 1024, // 25MB
                sync_writes: true,
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            OptimalCacheConfig {
                shard_count: 256,
                compression_level: 3,
                use_mmap: false,
                max_file_size: 50 * 1024 * 1024,
                sync_writes: true,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptimalCacheConfig {
    pub shard_count: usize,
    pub compression_level: u32,
    pub use_mmap: bool,
    pub max_file_size: usize,
    pub sync_writes: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_platform_optimizations() {
        let temp_dir = TempDir::new().unwrap();
        let result = PlatformOptimizations::apply_all(temp_dir.path());

        // Should not fail even if some optimizations can't be applied
        assert!(result.is_ok());
    }

    #[test]
    fn test_recommended_cache_path() {
        let path = PlatformOptimizations::recommended_cache_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_optimal_cache_config() {
        let config = PlatformOptimizations::optimal_cache_config();
        assert!(config.shard_count > 0);
        assert!(config.compression_level <= 9);
        assert!(config.max_file_size > 0);
    }
}
