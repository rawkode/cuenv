//! Streaming cache operations

mod get;
mod reader;
mod writer;

use crate::errors::Result;
use crate::streaming::StreamingCache;
use crate::streaming::{CacheReader, CacheWriter};
use futures::io::{AsyncRead, AsyncWrite};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::core::types::Cache;

impl StreamingCache for Cache {
    fn get_reader<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<CacheReader>>> + Send + 'a>> {
        self.get_reader(key)
    }

    fn get_writer<'a>(
        &'a self,
        key: &'a str,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<CacheWriter>> + Send + 'a>> {
        self.get_writer(key, ttl)
    }

    fn put_stream<'a, R>(
        &'a self,
        key: &'a str,
        reader: R,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>
    where
        R: AsyncRead + Send + 'a,
    {
        self.put_stream(key, reader, ttl)
    }

    fn get_stream<'a, W>(
        &'a self,
        key: &'a str,
        writer: W,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>>> + Send + 'a>>
    where
        W: AsyncWrite + Send + 'a,
    {
        self.get_stream(key, writer)
    }
}
