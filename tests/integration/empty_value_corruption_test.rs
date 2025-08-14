//! Test to isolate the corruption issue
//!
//! This test checks if storing and retrieving empty values causes problems

#[cfg(test)]
mod empty_value_tests {
    use cuenv::cache::{Cache, ProductionCache};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_empty_value_storage() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        // Store an empty vec
        let key = "test_empty_value";
        let empty_value: Vec<u8> = vec![];

        cache
            .put(key, &empty_value, None)
            .await
            .expect("Should be able to store empty value");

        // Try to retrieve it
        let retrieved: Option<Vec<u8>> = cache
            .get(key)
            .await
            .expect("Should be able to get empty value");

        assert_eq!(retrieved, Some(empty_value.clone()));

        // Now try to store something after the empty value
        let key2 = "after_empty";
        let value2 = vec![1, 2, 3];

        cache
            .put(key2, &value2, None)
            .await
            .expect("Should be able to store after empty value");

        let retrieved2: Option<Vec<u8>> = cache
            .get(key2)
            .await
            .expect("Should be able to get after empty value");

        assert_eq!(retrieved2, Some(value2));
    }
}
