//! Phase 2: Storage Backend Implementation
//!
//! This module provides a production-grade storage backend with:
//! - Binary format with bincode serialization
//! - Zstd compression for all cached data
//! - Write-ahead log for crash recovery
//! - CRC32C checksums for corruption detection
//! - Atomic multi-file updates
//! - Zero-copy operations where possible

use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp, StoreType};
use crate::traits::CacheMetadata;
use crc32c::crc32c;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read as IoRead, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::fs;
use tokio::sync::Semaphore;
use zstd::stream::{decode_all as zstd_decode, encode_all as zstd_encode};

/// Magic number for cache files: "CUEV" (CUEnV cache)
const CACHE_MAGIC: u32 = 0x43554556;

/// Current storage format version
const STORAGE_VERSION: u16 = 2;

/// Default zstd compression level (3 = fast with good compression)
const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

/// Maximum WAL size before rotation (10MB)
const MAX_WAL_SIZE: u64 = 10 * 1024 * 1024;

/// Binary storage header for all cache files
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C)]
struct StorageHeader {
    /// Magic number for validation
    magic: u32,
    /// Storage format version
    version: u16,
    /// Flags (bit 0: compressed, bit 1: encrypted, etc.)
    flags: u16,
    /// CRC32C of the header (excluding this field)
    header_crc: u32,
    /// Timestamp when written
    timestamp: u64,
    /// Uncompressed data size
    uncompressed_size: u64,
    /// Compressed data size (same as uncompressed if not compressed)
    compressed_size: u64,
    /// CRC32C of the data payload
    data_crc: u32,
    /// Reserved for future use
    reserved: [u8; 16],
}

impl StorageHeader {
    const FLAG_COMPRESSED: u16 = 1 << 0;
    #[allow(dead_code)]
    const FLAG_ENCRYPTED: u16 = 1 << 1;

    fn new(uncompressed_size: u64, compressed_size: u64, data_crc: u32, compressed: bool) -> Self {
        let mut header = Self {
            magic: CACHE_MAGIC,
            version: STORAGE_VERSION,
            flags: if compressed { Self::FLAG_COMPRESSED } else { 0 },
            header_crc: 0,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            uncompressed_size,
            compressed_size,
            data_crc,
            reserved: [0u8; 16],
        };

        // Calculate header CRC (excluding the CRC field itself)
        header.header_crc = header.calculate_crc();
        header
    }

    fn calculate_crc(&self) -> u32 {
        // Serialize header with CRC field set to 0
        let mut temp = *self;
        temp.header_crc = 0;

        let bytes = match bincode::serialize(&temp) {
            Ok(b) => b,
            Err(_) => return 0,
        };

        crc32c(&bytes)
    }

    fn validate(&self) -> Result<()> {
        // Check magic number
        if self.magic != CACHE_MAGIC {
            return Err(CacheError::Corruption {
                key: String::new(),
                reason: format!(
                    "Invalid magic number: expected {:08x}, got {:08x}",
                    CACHE_MAGIC, self.magic
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        // Check version
        if self.version > STORAGE_VERSION {
            return Err(CacheError::Corruption {
                key: String::new(),
                reason: format!("Unsupported storage version: {}", self.version),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Update cuenv to support newer cache format".to_string(),
                },
            });
        }

        // Verify header CRC
        let expected_crc = self.calculate_crc();
        if self.header_crc != expected_crc {
            return Err(CacheError::Corruption {
                key: String::new(),
                reason: format!(
                    "Header CRC mismatch: expected {:08x}, got {:08x}",
                    expected_crc, self.header_crc
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        Ok(())
    }

    fn is_compressed(&self) -> bool {
        self.flags & Self::FLAG_COMPRESSED != 0
    }
}

/// Write-Ahead Log entry type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOperation {
    /// Write a new cache entry
    Write {
        key: String,
        metadata_path: PathBuf,
        data_path: PathBuf,
        metadata: Vec<u8>,
        data: Vec<u8>,
    },
    /// Remove a cache entry
    Remove {
        key: String,
        metadata_path: PathBuf,
        data_path: PathBuf,
    },
    /// Clear all cache entries
    Clear,
    /// Checkpoint - all operations before this are committed
    Checkpoint { timestamp: SystemTime },
}

/// Write-Ahead Log for atomic operations
struct WriteAheadLog {
    /// Path to the WAL file
    path: PathBuf,
    /// Current WAL file handle
    file: Mutex<Option<BufWriter<File>>>,
    /// Current size of the WAL
    size: Arc<Mutex<u64>>,
    /// Sequence number for operations
    sequence: Arc<Mutex<u64>>,
}

impl WriteAheadLog {
    fn new(base_dir: &Path) -> Result<Self> {
        let wal_dir = base_dir.join("wal");
        match std::fs::create_dir_all(&wal_dir) {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: wal_dir.clone(),
                    operation: "create WAL directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: wal_dir },
                });
            }
        }

        let path = wal_dir.join("wal.log");
        let wal = Self {
            path: path.clone(),
            file: Mutex::new(None),
            size: Arc::new(Mutex::new(0)),
            sequence: Arc::new(Mutex::new(0)),
        };

        // Open or create the WAL file
        match wal.open_or_create() {
            Ok(()) => Ok(wal),
            Err(e) => Err(e),
        }
    }

    fn open_or_create(&self) -> Result<()> {
        let file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(f) => f,
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "open WAL file",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: self.path.clone(),
                    },
                });
            }
        };

        // Get current file size
        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "get WAL metadata",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        };

        *self.size.lock() = metadata.len();
        *self.file.lock() = Some(BufWriter::new(file));

        Ok(())
    }

    fn append(&self, op: &WalOperation) -> Result<u64> {
        let mut file_guard = self.file.lock();
        let file = match file_guard.as_mut() {
            Some(f) => f,
            None => {
                return Err(CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "WAL not initialized".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        };

        // Get next sequence number
        let seq = {
            let mut seq_guard = self.sequence.lock();
            *seq_guard += 1;
            *seq_guard
        };

        // Create WAL entry
        let entry = WalEntry {
            sequence: seq,
            timestamp: SystemTime::now(),
            operation: op.clone(),
            crc: 0,
        };

        // Serialize entry
        let mut entry_bytes = match bincode::serialize(&entry) {
            Ok(b) => b,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: String::new(),
                    operation: SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check WAL entry serialization".to_string(),
                    },
                });
            }
        };

        // Calculate and set CRC
        let crc = crc32c(&entry_bytes);
        if let Ok(mut entry_with_crc) = bincode::deserialize::<WalEntry>(&entry_bytes) {
            entry_with_crc.crc = crc;
            entry_bytes = match bincode::serialize(&entry_with_crc) {
                Ok(b) => b,
                Err(e) => {
                    return Err(CacheError::Serialization {
                        key: String::new(),
                        operation: SerializationOp::Encode,
                        source: Box::new(e),
                        recovery_hint: RecoveryHint::Manual {
                            instructions: "Check WAL entry serialization".to_string(),
                        },
                    });
                }
            };
        }

        // Write length prefix + entry
        let len_bytes = (entry_bytes.len() as u32).to_le_bytes();
        match file.write_all(&len_bytes) {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "write WAL length",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        match file.write_all(&entry_bytes) {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "write WAL entry",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        // Sync to disk for durability
        match file.flush() {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "flush WAL",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        // Update size
        let entry_size = 4 + entry_bytes.len() as u64;
        let mut size_guard = self.size.lock();
        *size_guard += entry_size;

        // Check if we need to rotate
        if *size_guard > MAX_WAL_SIZE {
            drop(size_guard);
            drop(file_guard);
            match self.rotate() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(seq)
    }

    fn rotate(&self) -> Result<()> {
        // Close current file
        *self.file.lock() = None;

        // Rename current WAL to backup
        let backup_path = self.path.with_extension(format!(
            "log.{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));

        match std::fs::rename(&self.path, &backup_path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "rotate WAL",
                    source: e,
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check disk space and permissions".to_string(),
                    },
                });
            }
        }

        // Open new WAL
        match self.open_or_create() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Write checkpoint to new WAL
        let checkpoint = WalOperation::Checkpoint {
            timestamp: SystemTime::now(),
        };

        match self.append(&checkpoint) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn replay<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(&WalOperation) -> Result<()>,
    {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(CacheError::Io {
                    path: self.path.clone(),
                    operation: "open WAL for replay",
                    source: e,
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check WAL file permissions".to_string(),
                    },
                });
            }
        };

        let mut reader = BufReader::new(file);
        let mut corrupted = false;

        loop {
            // Read length prefix
            let mut len_bytes = [0u8; 4];
            match reader.read_exact(&mut len_bytes) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    tracing::warn!("WAL replay error reading length: {}", e);
                    corrupted = true;
                    break;
                }
            }

            let len = u32::from_le_bytes(len_bytes) as usize;
            if len > 10 * 1024 * 1024 {
                tracing::warn!("WAL entry too large: {} bytes", len);
                corrupted = true;
                break;
            }

            // Read entry
            let mut entry_bytes = vec![0u8; len];
            match reader.read_exact(&mut entry_bytes) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!("WAL replay error reading entry: {}", e);
                    corrupted = true;
                    break;
                }
            }

            // Deserialize and verify
            let entry: WalEntry = match bincode::deserialize(&entry_bytes) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("WAL replay deserialization error: {}", e);
                    corrupted = true;
                    break;
                }
            };

            // Verify CRC
            let mut temp_entry = entry.clone();
            temp_entry.crc = 0;
            let temp_bytes = match bincode::serialize(&temp_entry) {
                Ok(b) => b,
                Err(_) => {
                    corrupted = true;
                    break;
                }
            };

            let expected_crc = crc32c(&temp_bytes);
            if entry.crc != expected_crc {
                tracing::warn!("WAL entry CRC mismatch");
                corrupted = true;
                break;
            }

            // Apply operation
            match callback(&entry.operation) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!("WAL replay callback error: {}", e);
                    // Continue replaying other entries
                }
            }
        }

        if corrupted {
            tracing::warn!("WAL corruption detected, truncating at last valid entry");
            // Could implement truncation here if needed
        }

        Ok(())
    }
}

/// WAL entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalEntry {
    sequence: u64,
    timestamp: SystemTime,
    operation: WalOperation,
    crc: u32,
}

/// Compression configuration
#[derive(Debug, Clone, Copy)]
pub struct CompressionConfig {
    /// Whether compression is enabled
    pub enabled: bool,
    /// Compression level (1-22 for zstd, default 3)
    pub level: i32,
    /// Minimum size in bytes before compression is applied
    pub min_size: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: DEFAULT_COMPRESSION_LEVEL,
            min_size: 1024, // Don't compress files smaller than 1KB
        }
    }
}

/// Storage backend for the cache system
pub struct StorageBackend {
    /// Base directory for storage
    #[allow(dead_code)]
    base_dir: PathBuf,
    /// Write-ahead log
    wal: Arc<WriteAheadLog>,
    /// Compression configuration
    compression: CompressionConfig,
    /// I/O semaphore for rate limiting
    io_semaphore: Arc<Semaphore>,
    /// Active transactions
    transactions: Arc<RwLock<HashMap<u64, Vec<WalOperation>>>>,
    /// Transaction counter
    tx_counter: Arc<Mutex<u64>>,
}

impl StorageBackend {
    /// Create a new storage backend
    pub async fn new(base_dir: PathBuf, compression: CompressionConfig) -> Result<Self> {
        // Create base directory
        match fs::create_dir_all(&base_dir).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: base_dir.clone(),
                    operation: "create storage directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: base_dir },
                });
            }
        }

        // Initialize WAL
        let wal = match WriteAheadLog::new(&base_dir) {
            Ok(w) => Arc::new(w),
            Err(e) => return Err(e),
        };

        let backend = Self {
            base_dir,
            wal,
            compression,
            io_semaphore: Arc::new(Semaphore::new(100)),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            tx_counter: Arc::new(Mutex::new(0)),
        };

        // Replay WAL to recover from any crashes
        match backend.recover().await {
            Ok(()) => Ok(backend),
            Err(e) => Err(e),
        }
    }

    /// Begin a new transaction
    pub fn begin_transaction(&self) -> u64 {
        let mut counter = self.tx_counter.lock();
        *counter += 1;
        let tx_id = *counter;

        self.transactions.write().insert(tx_id, Vec::new());
        tx_id
    }

    /// Add an operation to a transaction
    pub fn add_to_transaction(&self, tx_id: u64, op: WalOperation) -> Result<()> {
        let mut transactions = self.transactions.write();
        match transactions.get_mut(&tx_id) {
            Some(ops) => {
                ops.push(op);
                Ok(())
            }
            None => Err(CacheError::Configuration {
                message: format!("Transaction {tx_id} not found"),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Begin a transaction before adding operations".to_string(),
                },
            }),
        }
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, tx_id: u64) -> Result<()> {
        let ops = {
            let mut transactions = self.transactions.write();
            match transactions.remove(&tx_id) {
                Some(ops) => ops,
                None => {
                    return Err(CacheError::Configuration {
                        message: format!("Transaction {tx_id} not found"),
                        recovery_hint: RecoveryHint::Manual {
                            instructions: "Transaction may have already been committed".to_string(),
                        },
                    });
                }
            }
        };

        // Write all operations to WAL first
        for op in &ops {
            match self.wal.append(op) {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }

        // Execute all operations
        for op in ops {
            match self.execute_operation(&op).await {
                Ok(()) => {}
                Err(e) => {
                    // Log error but continue - WAL has the operation for retry
                    tracing::error!("Failed to execute operation: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback_transaction(&self, tx_id: u64) {
        self.transactions.write().remove(&tx_id);
    }

    /// Write data with compression and checksums
    pub async fn write(
        &self,
        path: &Path,
        data: &[u8],
        _metadata: Option<&CacheMetadata>,
    ) -> Result<()> {
        let _permit = match self.io_semaphore.acquire().await {
            Ok(p) => p,
            Err(_) => {
                return Err(CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "I/O semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        // Decide whether to compress
        let should_compress = self.compression.enabled && data.len() >= self.compression.min_size;

        tracing::debug!(
            "Write decision - path: {:?}, data_len: {}, min_size: {}, should_compress: {}",
            path,
            data.len(),
            self.compression.min_size,
            should_compress
        );

        // Compress if needed
        let (compressed_data, compressed_size, uncompressed_size) = if should_compress {
            match zstd_encode(data, self.compression.level) {
                Ok(compressed) => {
                    let compressed_len = compressed.len();
                    tracing::debug!(
                        "Compressed data - original: {}, compressed: {}",
                        data.len(),
                        compressed_len
                    );
                    (compressed, compressed_len as u64, data.len() as u64)
                }
                Err(e) => {
                    return Err(CacheError::Compression {
                        operation: "compress",
                        source: Box::new(e),
                        recovery_hint: RecoveryHint::Manual {
                            instructions: "Check compression settings".to_string(),
                        },
                    });
                }
            }
        } else {
            (data.to_vec(), data.len() as u64, data.len() as u64)
        };

        // Calculate data CRC
        let data_crc = crc32c(&compressed_data);

        // Create header
        let header = StorageHeader::new(
            uncompressed_size,
            compressed_size,
            data_crc,
            should_compress,
        );

        // Serialize header
        let header_bytes = match bincode::serialize(&header) {
            Ok(b) => b,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: path.to_string_lossy().to_string(),
                    operation: SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check header serialization".to_string(),
                    },
                });
            }
        };

        // Write atomically
        let temp_path = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));

        // Ensure parent directory exists
        if let Some(parent) = temp_path.parent() {
            match fs::create_dir_all(parent).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: parent.to_path_buf(),
                        operation: "create parent directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: parent.to_path_buf(),
                        },
                    });
                }
            }
        }

        // Write header + data
        let mut output = Vec::with_capacity(header_bytes.len() + compressed_data.len());
        output.extend_from_slice(&header_bytes);
        output.extend_from_slice(&compressed_data);

        match fs::write(&temp_path, &output).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: temp_path.clone(),
                    operation: "write cache file",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: temp_path },
                });
            }
        }

        // Atomic rename
        match fs::rename(&temp_path, path).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Clean up temp file
                let _ = fs::remove_file(&temp_path).await;
                Err(CacheError::Io {
                    path: path.to_path_buf(),
                    operation: "atomic rename",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                })
            }
        }
    }

    /// Read data with decompression and checksum verification
    pub async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let _permit = match self.io_semaphore.acquire().await {
            Ok(p) => p,
            Err(_) => {
                return Err(CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "I/O semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        // Read file
        let file_data = match fs::read(path).await {
            Ok(d) => d,
            Err(e) => {
                return Err(CacheError::Io {
                    path: path.to_path_buf(),
                    operation: "read cache file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        // Deserialize header - bincode uses variable length encoding
        // So we need to deserialize from the beginning and let bincode determine the size
        let header: StorageHeader = match bincode::deserialize(&file_data) {
            Ok(h) => h,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: path.to_string_lossy().to_string(),
                    operation: SerializationOp::Decode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::ClearAndRetry,
                });
            }
        };

        // Validate header
        match header.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Calculate header size by serializing it again
        let header_bytes = match bincode::serialize(&header) {
            Ok(b) => b,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: path.to_string_lossy().to_string(),
                    operation: SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::ClearAndRetry,
                });
            }
        };
        let header_size = header_bytes.len();

        // Extract data portion
        if file_data.len() < header_size {
            return Err(CacheError::Corruption {
                key: path.to_string_lossy().to_string(),
                reason: "File too small after header".to_string(),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }
        let data = &file_data[header_size..];

        // Verify data CRC
        let actual_crc = crc32c(data);

        tracing::debug!(
            "Read validation - path: {:?}, is_compressed: {}, data_len: {}, expected_crc: {:08x}, actual_crc: {:08x}",
            path, header.is_compressed(), data.len(), header.data_crc, actual_crc
        );

        if actual_crc != header.data_crc {
            return Err(CacheError::Corruption {
                key: path.to_string_lossy().to_string(),
                reason: format!(
                    "Data CRC mismatch: expected {:08x}, got {:08x}",
                    header.data_crc, actual_crc
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        // Decompress if needed
        if header.is_compressed() {
            match zstd_decode(data) {
                Ok(decompressed) => Ok(decompressed),
                Err(e) => Err(CacheError::Compression {
                    operation: "decompress",
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::ClearAndRetry,
                }),
            }
        } else {
            Ok(data.to_vec())
        }
    }

    /// Execute a WAL operation
    async fn execute_operation(&self, op: &WalOperation) -> Result<()> {
        match op {
            WalOperation::Write {
                key: _,
                metadata_path,
                data_path,
                metadata,
                data,
            } => {
                // Write metadata
                match self.write(metadata_path, metadata, None).await {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                }

                // Write data
                match self.write(data_path, data, None).await {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        // Try to clean up metadata
                        let _ = fs::remove_file(metadata_path).await;
                        Err(e)
                    }
                }
            }
            WalOperation::Remove {
                key: _,
                metadata_path,
                data_path,
            } => {
                let _ = fs::remove_file(metadata_path).await;
                let _ = fs::remove_file(data_path).await;
                Ok(())
            }
            WalOperation::Clear => {
                // Clear is handled at a higher level
                Ok(())
            }
            WalOperation::Checkpoint { timestamp: _ } => {
                // Checkpoint is just a marker
                Ok(())
            }
        }
    }

    /// Recover from WAL on startup
    async fn recover(&self) -> Result<()> {
        // Collect operations first to avoid sync/async mixing
        let mut operations = Vec::new();

        self.wal.replay(|op| {
            operations.push(op.clone());
            Ok(())
        })?;

        // Process operations asynchronously
        for op in operations {
            let _permit =
                self.io_semaphore
                    .acquire()
                    .await
                    .map_err(|_| CacheError::ConcurrencyConflict {
                        key: "wal_recovery".to_string(),
                        operation: "acquire_semaphore",
                        duration: Duration::from_secs(0),
                        recovery_hint: RecoveryHint::Retry {
                            after: Duration::from_millis(100),
                        },
                    })?;

            match op {
                WalOperation::Write {
                    key: _,
                    metadata_path,
                    data_path,
                    metadata,
                    data,
                } => {
                    // Just write the files directly during recovery
                    let _ = fs::write(&metadata_path, &metadata).await;
                    let _ = fs::write(&data_path, &data).await;
                }
                WalOperation::Remove {
                    key: _,
                    metadata_path,
                    data_path,
                } => {
                    let _ = fs::remove_file(&metadata_path).await;
                    let _ = fs::remove_file(&data_path).await;
                }
                WalOperation::Clear => {
                    // Clear operation would be handled at cache level
                }
                WalOperation::Checkpoint { timestamp: _ } => {
                    // Nothing to do for checkpoint during recovery
                }
            }
        }

        Ok(())
    }

    /// Write data to cache with WAL support
    pub async fn write_cache_entry(
        &self,
        key: &str,
        metadata_path: &Path,
        data_path: &Path,
        metadata: &CacheMetadata,
        data: &[u8],
    ) -> Result<()> {
        // Serialize metadata
        let metadata_bytes = match bincode::serialize(metadata) {
            Ok(b) => b,
            Err(e) => {
                return Err(CacheError::Serialization {
                    key: key.to_string(),
                    operation: SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check metadata serialization".to_string(),
                    },
                });
            }
        };

        // Create WAL operation
        let wal_op = WalOperation::Write {
            key: key.to_string(),
            metadata_path: metadata_path.to_path_buf(),
            data_path: data_path.to_path_buf(),
            metadata: metadata_bytes.clone(),
            data: data.to_vec(),
        };

        // Append to WAL first
        match self.wal.append(&wal_op) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        // Write metadata
        match self
            .write(metadata_path, &metadata_bytes, Some(metadata))
            .await
        {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Write data
        match self.write(data_path, data, Some(metadata)).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Try to clean up metadata
                let _ = fs::remove_file(metadata_path).await;
                Err(e)
            }
        }
    }

    /// Remove cache entry with WAL support
    pub async fn remove_cache_entry(
        &self,
        key: &str,
        metadata_path: &Path,
        data_path: &Path,
    ) -> Result<()> {
        // Create WAL operation
        let wal_op = WalOperation::Remove {
            key: key.to_string(),
            metadata_path: metadata_path.to_path_buf(),
            data_path: data_path.to_path_buf(),
        };

        // Append to WAL first
        match self.wal.append(&wal_op) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        // Remove files
        let _ = fs::remove_file(metadata_path).await;
        let _ = fs::remove_file(data_path).await;

        Ok(())
    }

    /// Get compression statistics
    pub fn compression_stats(&self) -> CompressionStats {
        CompressionStats {
            enabled: self.compression.enabled,
            level: self.compression.level,
            min_size: self.compression.min_size,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub enabled: bool,
    pub level: i32,
    pub min_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_backend_basic() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let backend =
            StorageBackend::new(temp_dir.path().to_path_buf(), CompressionConfig::default())
                .await?;

        let test_path = temp_dir.path().join("test.bin");
        let test_data = b"Hello, World!";

        // Write data
        match backend.write(&test_path, test_data, None).await {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Read data back
        let read_data = match backend.read(&test_path).await {
            Ok(d) => d,
            Err(e) => return Err(e),
        };

        assert_eq!(read_data, test_data);

        Ok(())
    }

    #[tokio::test]
    async fn test_compression() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let backend = StorageBackend::new(
            temp_dir.path().to_path_buf(),
            CompressionConfig {
                enabled: true,
                level: 3,
                min_size: 10,
            },
        )
        .await?;

        let test_path = temp_dir.path().join("compressed.bin");

        // Create compressible data
        let test_data = vec![b'A'; 10000];

        // Write data
        match backend.write(&test_path, &test_data, None).await {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Check file size is smaller than original
        let file_meta = std::fs::metadata(&test_path).unwrap();
        assert!(file_meta.len() < test_data.len() as u64);

        // Read data back
        let read_data = match backend.read(&test_path).await {
            Ok(d) => d,
            Err(e) => return Err(e),
        };

        assert_eq!(read_data, test_data);

        Ok(())
    }

    #[tokio::test]
    async fn test_corruption_detection() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let backend =
            StorageBackend::new(temp_dir.path().to_path_buf(), CompressionConfig::default())
                .await?;

        let test_path = temp_dir.path().join("corrupt.bin");
        let test_data = b"Test data for corruption";

        // Write data
        match backend.write(&test_path, test_data, None).await {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Corrupt the file
        let mut file_data = std::fs::read(&test_path).unwrap();
        // Deserialize header to find data start
        let header: StorageHeader = bincode::deserialize(&file_data).unwrap();
        let header_bytes = bincode::serialize(&header).unwrap();
        let data_start = header_bytes.len();
        // Flip some bits in the data portion
        if file_data.len() > data_start + 10 {
            file_data[data_start + 5] ^= 0xFF;
        }
        std::fs::write(&test_path, file_data).unwrap();

        // Try to read - should detect corruption
        match backend.read(&test_path).await {
            Ok(_) => panic!("Should have detected corruption"),
            Err(CacheError::Corruption { .. }) => {}
            Err(e) => panic!("Wrong error type: {e}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_wal_recovery() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Create backend and write some data
        {
            let backend = StorageBackend::new(path.clone(), CompressionConfig::default()).await?;

            let key = "test_key";
            let metadata_path = path.join("test.meta");
            let data_path = path.join("test.data");
            let metadata = CacheMetadata {
                created_at: SystemTime::now(),
                last_accessed: SystemTime::now(),
                expires_at: None,
                size_bytes: 13,
                access_count: 0,
                content_hash: "test_hash".to_string(),
                cache_version: 1,
            };

            match backend
                .write_cache_entry(key, &metadata_path, &data_path, &metadata, b"Test WAL data")
                .await
            {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }

        // Simulate crash by removing the data file but keeping WAL
        let data_path = path.join("test.data");
        std::fs::remove_file(&data_path).ok();

        // Create new backend - should recover from WAL
        let _backend2 = StorageBackend::new(path.clone(), CompressionConfig::default()).await?;

        // Data should be recovered
        assert!(data_path.exists(), "Data file should be recovered from WAL");

        Ok(())
    }
}
