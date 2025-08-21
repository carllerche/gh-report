use anyhow::{Context, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

mod compression;
mod key_gen;
mod storage;

pub use compression::{compress_data, decompress_data};
pub use key_gen::{generate_cache_key, CacheKeyBuilder};
pub use storage::{CacheEntry, CacheStorage};

/// Main cache manager
pub struct CacheManager {
    cache_dir: PathBuf,
    ttl_hours: u32,
    compression_enabled: bool,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(cache_dir: PathBuf, ttl_hours: u32, compression_enabled: bool) -> Self {
        CacheManager {
            cache_dir,
            ttl_hours,
            compression_enabled,
        }
    }

    /// Initialize cache directory structure
    pub fn initialize(&self) -> Result<()> {
        // Create cache subdirectories
        let subdirs = ["github", "claude", "contexts", "temp"];

        for subdir in &subdirs {
            let path = self.cache_dir.join(subdir);
            fs::create_dir_all(&path)
                .with_context(|| format!("Failed to create cache directory: {:?}", path))?;
        }

        info!("Cache initialized at {:?}", self.cache_dir);
        Ok(())
    }

    /// Get cached GitHub response
    pub fn get_github_response(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.cache_dir.join("github").join(format!("{}.cache", key));
        self.get_cached_data(&path)
    }

    /// Cache GitHub response
    pub fn cache_github_response(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.cache_dir.join("github").join(format!("{}.cache", key));
        self.cache_data(&path, data)
    }

    /// Get cached Claude response
    pub fn get_claude_response(&self, key: &str) -> Result<Option<String>> {
        let path = self.cache_dir.join("claude").join(format!("{}.cache", key));
        if let Some(data) = self.get_cached_data(&path)? {
            String::from_utf8(data)
                .map(Some)
                .context("Invalid UTF-8 in cached Claude response")
        } else {
            Ok(None)
        }
    }

    /// Cache Claude response
    pub fn cache_claude_response(&self, key: &str, response: &str) -> Result<()> {
        let path = self.cache_dir.join("claude").join(format!("{}.cache", key));
        self.cache_data(&path, response.as_bytes())
    }

    /// Get cached context for an issue/PR
    pub fn get_issue_context(&self, repo: &str, issue_number: u32) -> Result<Option<IssueContext>> {
        let key = format!("{}_{}", repo.replace('/', "_"), issue_number);
        let path = self
            .cache_dir
            .join("contexts")
            .join(format!("{}.json", key));

        if path.exists() {
            let data = fs::read(&path)
                .with_context(|| format!("Failed to read context cache: {:?}", path))?;

            let context: IssueContext =
                serde_json::from_slice(&data).context("Failed to deserialize issue context")?;

            // Check if context is still valid
            if self.is_valid_timestamp(context.cached_at) {
                Ok(Some(context))
            } else {
                // Context expired, remove it
                let _ = fs::remove_file(&path);
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Cache issue context
    pub fn cache_issue_context(
        &self,
        repo: &str,
        issue_number: u32,
        context: &IssueContext,
    ) -> Result<()> {
        let key = format!("{}_{}", repo.replace('/', "_"), issue_number);
        let path = self
            .cache_dir
            .join("contexts")
            .join(format!("{}.json", key));

        let data =
            serde_json::to_vec_pretty(context).context("Failed to serialize issue context")?;

        fs::write(&path, data)
            .with_context(|| format!("Failed to write context cache: {:?}", path))?;

        Ok(())
    }

    /// Clear all cache
    pub fn clear_all(&self) -> Result<()> {
        info!("Clearing all cache at {:?}", self.cache_dir);

        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)
                .with_context(|| format!("Failed to clear cache: {:?}", self.cache_dir))?;
        }

        // Reinitialize
        self.initialize()?;

        Ok(())
    }

    /// Clear expired cache entries
    pub fn clear_expired(&self) -> Result<usize> {
        let mut removed = 0;

        for subdir in &["github", "claude", "contexts"] {
            let dir = self.cache_dir.join(subdir);
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            let age = modified.elapsed().unwrap_or_default();
                            let max_age =
                                std::time::Duration::from_secs((self.ttl_hours as u64) * 3600);

                            if age > max_age {
                                debug!("Removing expired cache: {:?}", path);
                                let _ = fs::remove_file(&path);
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }

        if removed > 0 {
            info!("Removed {} expired cache entries", removed);
        }

        Ok(removed)
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> Result<CacheStats> {
        let mut stats = CacheStats::default();

        for subdir in &["github", "claude", "contexts"] {
            let dir = self.cache_dir.join(subdir);
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let metadata = entry.metadata()?;

                if metadata.is_file() {
                    stats.total_entries += 1;
                    stats.total_size += metadata.len();

                    match subdir.as_ref() {
                        "github" => stats.github_entries += 1,
                        "claude" => stats.claude_entries += 1,
                        "contexts" => stats.context_entries += 1,
                        _ => {}
                    }
                }
            }
        }

        Ok(stats)
    }

    // Helper methods

    fn get_cached_data(&self, path: &Path) -> Result<Option<Vec<u8>>> {
        if !path.exists() {
            return Ok(None);
        }

        // Check if cache is still valid
        let metadata = fs::metadata(path)?;
        if let Ok(modified) = metadata.modified() {
            let age = modified.elapsed().unwrap_or_default();
            let max_age = std::time::Duration::from_secs((self.ttl_hours as u64) * 3600);

            if age > max_age {
                debug!("Cache expired: {:?}", path);
                let _ = fs::remove_file(path);
                return Ok(None);
            }
        }

        let data = fs::read(path).with_context(|| format!("Failed to read cache: {:?}", path))?;

        if self.compression_enabled {
            decompress_data(&data).map(Some)
        } else {
            Ok(Some(data))
        }
    }

    fn cache_data(&self, path: &Path, data: &[u8]) -> Result<()> {
        let data_to_store = if self.compression_enabled {
            compress_data(data)?
        } else {
            data.to_vec()
        };

        fs::write(path, data_to_store)
            .with_context(|| format!("Failed to write cache: {:?}", path))?;

        debug!("Cached data to {:?}", path);
        Ok(())
    }

    fn is_valid_timestamp(&self, timestamp: Timestamp) -> bool {
        let now = Timestamp::now();
        let age_hours = ((now - timestamp).get_hours() as u32).max(0);
        age_hours < self.ttl_hours
    }
}

/// Cached issue context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueContext {
    pub issue_number: u32,
    pub repo: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub last_processed_comment_id: Option<u64>,
    pub cached_at: Timestamp,
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size: u64,
    pub github_entries: usize,
    pub claude_entries: usize,
    pub context_entries: usize,
}

impl CacheStats {
    /// Get human-readable size
    pub fn size_human(&self) -> String {
        let size = self.total_size as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.2} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CacheManager::new(temp_dir.path().to_path_buf(), 24, false);

        assert!(manager.initialize().is_ok());
        assert!(temp_dir.path().join("github").exists());
        assert!(temp_dir.path().join("claude").exists());
        assert!(temp_dir.path().join("contexts").exists());
    }

    #[test]
    fn test_cache_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CacheManager::new(temp_dir.path().to_path_buf(), 24, false);
        manager.initialize().unwrap();

        // Test GitHub cache
        let key = "test_key";
        let data = b"test data";

        manager.cache_github_response(key, data).unwrap();
        let retrieved = manager.get_github_response(key).unwrap();

        assert_eq!(retrieved, Some(data.to_vec()));
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CacheManager::new(temp_dir.path().to_path_buf(), 24, false);
        manager.initialize().unwrap();

        // Add some cache entries
        manager.cache_github_response("test1", b"data1").unwrap();
        manager.cache_claude_response("test2", "data2").unwrap();

        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.github_entries, 1);
        assert_eq!(stats.claude_entries, 1);
    }
}
