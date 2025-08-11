//! Tests for streaming cache operations

use super::*;
use crate::traits::CacheMetadata;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use sha2::{Digest, Sha256};
use std::time::SystemTime;
use tempfile::TempDir;

#[tokio::test]
async fn test_streaming_write_read() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create necessary directories
    tokio::fs::create_dir_all(cache_dir.join("objects"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(cache_dir.join("metadata"))
        .await
        .unwrap();

    // Write data using streaming API
    let mut writer = CacheWriter::new(&cache_dir, "test_key", None).await?;
    writer.write_all(b"Hello, streaming cache!").await.unwrap();
    let metadata = writer.finalize().await?;

    assert_eq!(metadata.size_bytes, 23);

    // Read data back
    let hash = CacheWriter::hash_key("test_key");
    let (data_path, _) = CacheWriter::get_paths(&cache_dir, &hash);

    let mut reader = CacheReader::from_file(data_path, metadata).await?;
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await.unwrap();

    assert_eq!(&buffer, b"Hello, streaming cache!");
    assert!(reader.verify_integrity());

    Ok(())
}

#[tokio::test]
async fn test_large_streaming_copy() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create necessary directories
    tokio::fs::create_dir_all(cache_dir.join("objects"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(cache_dir.join("metadata"))
        .await
        .unwrap();

    // Create large test data (10MB)
    let large_data = vec![0x42u8; 10 * 1024 * 1024];

    // Write using streaming
    let mut writer = CacheWriter::new(&cache_dir, "large_key", None).await?;
    writer.write_all(&large_data).await.unwrap();
    let metadata = writer.finalize().await?;

    assert_eq!(metadata.size_bytes, large_data.len() as u64);

    // Read back and verify
    let hash = CacheWriter::hash_key("large_key");
    let (data_path, _) = CacheWriter::get_paths(&cache_dir, &hash);

    let mut reader = CacheReader::from_file(data_path, metadata).await?;
    let mut read_data = Vec::new();
    reader.read_to_end(&mut read_data).await.unwrap();

    assert_eq!(read_data.len(), large_data.len());
    assert!(reader.verify_integrity());

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_mmap_reader() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.dat");

    // Write test data
    tokio::fs::write(&test_file, b"Memory mapped data")
        .await
        .unwrap();

    let metadata = CacheMetadata {
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: 18,
        access_count: 0,
        content_hash: {
            let mut hasher = Sha256::new();
            hasher.update(b"Memory mapped data");
            format!("{:x}", hasher.finalize())
        },
        cache_version: 3,
    };

    // Read using mmap
    let mut reader = CacheReader::from_mmap(test_file, metadata).await?;
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await.unwrap();

    assert_eq!(&buffer, b"Memory mapped data");
    assert!(reader.verify_integrity());

    Ok(())
}

#[tokio::test]
async fn test_memory_reader() {
    let test_data = b"Test memory data";
    let metadata = CacheMetadata {
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: test_data.len() as u64,
        access_count: 0,
        content_hash: {
            let mut hasher = Sha256::new();
            hasher.update(test_data);
            format!("{:x}", hasher.finalize())
        },
        cache_version: 3,
    };

    let mut reader = CacheReader::from_memory(test_data.to_vec(), metadata);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await.unwrap();

    assert_eq!(&buffer, test_data);
    assert!(reader.verify_integrity());
}

#[tokio::test]
async fn test_writer_public_api() {
    // Test that public API wrappers still work correctly
    let cache_dir = std::path::Path::new("/tmp/cache");
    let hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";

    let (data_path, metadata_path) = CacheWriter::get_paths(cache_dir, hash);

    assert_eq!(data_path, cache_dir.join("objects").join("ab").join(hash));

    assert_eq!(
        metadata_path,
        cache_dir
            .join("metadata")
            .join("ab")
            .join(format!("{hash}.meta"))
    );

    // Test key hashing wrapper
    let key = "test_key";
    let hash1 = CacheWriter::hash_key(key);
    let hash2 = CacheWriter::hash_key(key);

    // Same key should produce same hash
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // SHA256 hex string length
}
