use anyhow::{Context, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::debug;

/// Cache storage implementation
pub struct CacheStorage {
    base_dir: PathBuf,
}

impl CacheStorage {
    /// Create new cache storage
    pub fn new(base_dir: PathBuf) -> Self {
        CacheStorage { base_dir }
    }

    /// Get path for a cache key
    pub fn get_path(&self, namespace: &str, key: &str) -> PathBuf {
        self.base_dir.join(namespace).join(format!("{}.cache", key))
    }

    /// Check if cache entry exists
    pub fn exists(&self, namespace: &str, key: &str) -> bool {
        self.get_path(namespace, key).exists()
    }

    /// Write cache entry
    pub fn write(&self, namespace: &str, key: &str, data: &[u8]) -> Result<()> {
        let path = self.get_path(namespace, key);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create cache directory: {:?}", parent))?;
        }

        fs::write(&path, data).with_context(|| format!("Failed to write cache: {:?}", path))?;

        debug!("Wrote cache entry: {:?}", path);
        Ok(())
    }

    /// Read cache entry
    pub fn read(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.get_path(namespace, key);

        if !path.exists() {
            return Ok(None);
        }

        let data = fs::read(&path).with_context(|| format!("Failed to read cache: {:?}", path))?;

        Ok(Some(data))
    }

    /// Delete cache entry
    pub fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let path = self.get_path(namespace, key);

        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete cache: {:?}", path))?;
            debug!("Deleted cache entry: {:?}", path);
        }

        Ok(())
    }

    /// List all entries in a namespace
    pub fn list_entries(&self, namespace: &str) -> Result<Vec<String>> {
        let dir = self.base_dir.join(namespace);

        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_stem() {
                    if let Some(name_str) = name.to_str() {
                        entries.push(name_str.to_string());
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Clear all entries in a namespace
    pub fn clear_namespace(&self, namespace: &str) -> Result<usize> {
        let dir = self.base_dir.join(namespace);

        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                fs::remove_file(&path)?;
                count += 1;
            }
        }

        Ok(count)
    }
}

/// Cache entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub data: Vec<u8>,
    pub created_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub metadata: CacheMetadata,
}

/// Cache metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheMetadata {
    pub content_type: Option<String>,
    pub encoding: Option<String>,
    pub compressed: bool,
    pub size_bytes: usize,
    pub checksum: Option<String>,
}

impl CacheEntry {
    /// Create new cache entry
    pub fn new(key: String, data: Vec<u8>) -> Self {
        let metadata = CacheMetadata {
            size_bytes: data.len(),
            ..Default::default()
        };

        CacheEntry {
            key,
            data,
            created_at: Timestamp::now(),
            expires_at: None,
            metadata,
        }
    }

    /// Set expiration time
    pub fn with_expiration(mut self, expires_at: Timestamp) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set compression flag
    pub fn with_compression(mut self, compressed: bool) -> Self {
        self.metadata.compressed = compressed;
        self
    }

    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Timestamp::now() > expires_at
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_storage_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = CacheStorage::new(temp_dir.path().to_path_buf());

        // Test write and read
        let namespace = "test";
        let key = "test_key";
        let data = b"test data";

        storage.write(namespace, key, data).unwrap();
        assert!(storage.exists(namespace, key));

        let read_data = storage.read(namespace, key).unwrap();
        assert_eq!(read_data, Some(data.to_vec()));

        // Test list entries
        storage.write(namespace, "key2", b"data2").unwrap();
        let entries = storage.list_entries(namespace).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&"test_key".to_string()));
        assert!(entries.contains(&"key2".to_string()));

        // Test delete
        storage.delete(namespace, key).unwrap();
        assert!(!storage.exists(namespace, key));

        // Test clear namespace
        let cleared = storage.clear_namespace(namespace).unwrap();
        assert_eq!(cleared, 1);
        let entries = storage.list_entries(namespace).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_cache_entry() {
        let entry = CacheEntry::new("key".to_string(), vec![1, 2, 3]);

        assert_eq!(entry.key, "key");
        assert_eq!(entry.data, vec![1, 2, 3]);
        assert_eq!(entry.metadata.size_bytes, 3);
        assert!(!entry.is_expired());

        // Test with expiration
        let future = Timestamp::now() + jiff::ToSpan::hours(1);
        let entry = entry.with_expiration(future);
        assert!(!entry.is_expired());

        let past = Timestamp::now() - jiff::ToSpan::hours(1);
        let expired_entry = CacheEntry::new("key".to_string(), vec![]).with_expiration(past);
        assert!(expired_entry.is_expired());
    }
}
