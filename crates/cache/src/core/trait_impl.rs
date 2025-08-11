//! CacheTrait implementation for Cache

use crate::errors::Result;
use crate::traits::{Cache as CacheTrait, CacheMetadata, CacheStatistics};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

use super::types::Cache;

#[async_trait]
impl CacheTrait for Cache {
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        self.get(key).await
    }

    async fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        self.put(key, value, ttl).await
    }

    async fn remove(&self, key: &str) -> Result<bool> {
        self.remove(key).await
    }

    async fn contains(&self, key: &str) -> Result<bool> {
        self.contains(key).await
    }

    async fn metadata(&self, key: &str) -> Result<Option<CacheMetadata>> {
        self.metadata(key).await
    }

    async fn clear(&self) -> Result<()> {
        self.clear().await
    }

    async fn statistics(&self) -> Result<CacheStatistics> {
        self.statistics().await
    }
}