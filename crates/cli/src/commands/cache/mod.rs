use clap::Subcommand;
use cuenv_cache::{CacheConfig, CacheManager};
use cuenv_core::Result;

#[derive(Subcommand)]
pub enum CacheCommands {
    /// Clear all cache entries
    Clear,
    /// Show cache statistics
    Stats,
    /// Clean up stale cache entries
    Cleanup {
        /// Maximum age of cache entries to keep (in hours)
        #[arg(long, default_value = "168")]
        max_age_hours: u64,
    },
}

impl CacheCommands {
    pub async fn execute(self) -> Result<()> {
        match self {
            CacheCommands::Clear => {
                let config = CacheConfig::default();
                let manager = CacheManager::new(config).await?;
                manager.clear_cache()?;
                println!("✓ Cache cleared successfully");
                Ok(())
            }
            CacheCommands::Stats => {
                let config = CacheConfig::default();
                let manager = CacheManager::new(config).await?;
                let stats = manager.get_statistics();
                println!("Cache Statistics:");
                println!("  Hits: {}", stats.hits);
                println!("  Misses: {}", stats.misses);
                println!("  Writes: {}", stats.writes);
                println!("  Errors: {}", stats.errors);
                let hit_rate = if stats.hits + stats.misses > 0 {
                    (stats.hits as f64 / (stats.hits + stats.misses) as f64) * 100.0
                } else {
                    0.0
                };
                println!("  Hit rate: {hit_rate:.1}%");
                println!(
                    "  Total bytes saved: {:.2} MB",
                    stats.total_bytes_saved as f64 / 1_048_576.0
                );
                Ok(())
            }
            CacheCommands::Cleanup { max_age_hours: _ } => {
                let config = CacheConfig::default();
                let manager = CacheManager::new(config).await?;
                manager.cleanup_stale_entries()?;
                println!("✓ Cleaned up stale cache entries");
                Ok(())
            }
        }
    }
}
