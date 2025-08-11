//! Tests for the storage backend
//!
//! This module contains all tests for the storage system including
//! compression, corruption detection, and WAL recovery.

#[cfg(test)]
mod storage_tests {
    use crate::errors::CacheError;
    use crate::storage::{CompressionConfig, StorageBackend, StorageHeader};
    use crate::traits::CacheMetadata;
    use crate::Result;
    use std::time::SystemTime;
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
