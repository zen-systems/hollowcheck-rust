// hollowcheck:ignore-file mock_data - Test fixtures contain fake IDs
//! File-based cache for registry lookup results.
//!
//! Caches both positive (exists) and negative (404) results to avoid
//! repeated network calls. Cache is stored in ~/.cache/hollowcheck/registry/

use super::{PackageStatus, RegistryType};
use directories::ProjectDirs;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// In-memory + file-based cache for registry results.
pub struct RegistryCache {
    /// In-memory cache for current session
    memory: RwLock<HashMap<String, CacheEntry>>,
    /// Path to cache directory
    cache_dir: Option<PathBuf>,
    /// TTL in hours
    ttl_hours: u32,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    status: PackageStatus,
    timestamp: u64, // Unix timestamp in seconds
}

impl RegistryCache {
    /// Create a new registry cache with the given TTL.
    pub fn new(ttl_hours: u32) -> Self {
        let cache_dir =
            ProjectDirs::from("", "", "hollowcheck").map(|dirs| dirs.cache_dir().join("registry"));

        // Try to create cache directory
        if let Some(ref dir) = cache_dir {
            let _ = fs::create_dir_all(dir);
        }

        Self {
            memory: RwLock::new(HashMap::new()),
            cache_dir,
            ttl_hours,
        }
    }

    /// Generate a cache key for a registry/package pair.
    fn cache_key(registry: RegistryType, package: &str) -> String {
        format!("{}:{}", registry.as_str(), package)
    }

    /// Get a cached result if it exists and is not expired.
    pub fn get(&self, registry: RegistryType, package: &str) -> Option<PackageStatus> {
        let key = Self::cache_key(registry, package);
        let now = current_timestamp();
        let ttl_secs = (self.ttl_hours as u64) * 3600;

        // Check in-memory cache first
        {
            let cache = self.memory.read().ok()?;
            if let Some(entry) = cache.get(&key) {
                if now - entry.timestamp < ttl_secs {
                    return Some(entry.status.clone());
                }
            }
        }

        // Check file cache
        if let Some(entry) = self.read_file_cache(&key) {
            if now - entry.timestamp < ttl_secs {
                // Promote to memory cache
                if let Ok(mut cache) = self.memory.write() {
                    cache.insert(key, entry.clone());
                }
                return Some(entry.status);
            }
        }

        None
    }

    /// Store a result in the cache.
    pub fn set(&self, registry: RegistryType, package: &str, status: PackageStatus) {
        let key = Self::cache_key(registry, package);
        let entry = CacheEntry {
            status: status.clone(),
            timestamp: current_timestamp(),
        };

        // Store in memory
        if let Ok(mut cache) = self.memory.write() {
            cache.insert(key.clone(), entry.clone());
        }

        // Store to file
        self.write_file_cache(&key, &entry);
    }

    /// Read from file cache.
    fn read_file_cache(&self, key: &str) -> Option<CacheEntry> {
        let path = self.cache_file_path(key)?;
        let content = fs::read_to_string(path).ok()?;
        parse_cache_entry(&content)
    }

    /// Write to file cache.
    fn write_file_cache(&self, key: &str, entry: &CacheEntry) {
        if let Some(path) = self.cache_file_path(key) {
            let content = format_cache_entry(entry);
            let _ = fs::write(path, content);
        }
    }

    /// Get the file path for a cache key.
    fn cache_file_path(&self, key: &str) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|dir| {
            // Sanitize key for filename (replace : with _)
            let filename = key.replace([':', '/'], "_");
            dir.join(format!("{}.cache", filename))
        })
    }
}

/// Get current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Format a cache entry for file storage.
fn format_cache_entry(entry: &CacheEntry) -> String {
    let status_str = match &entry.status {
        PackageStatus::Exists => "exists",
        PackageStatus::NotFound => "notfound",
        PackageStatus::Unknown(msg) => return format!("unknown:{}:{}", entry.timestamp, msg),
    };
    format!("{}:{}", status_str, entry.timestamp)
}

/// Parse a cache entry from file content.
fn parse_cache_entry(content: &str) -> Option<CacheEntry> {
    let content = content.trim();
    let parts: Vec<&str> = content.splitn(3, ':').collect();

    if parts.len() < 2 {
        return None;
    }

    let timestamp = parts[1].parse().ok()?;
    let status = match parts[0] {
        "exists" => PackageStatus::Exists,
        "notfound" => PackageStatus::NotFound,
        "unknown" => {
            let msg = parts.get(2).unwrap_or(&"unknown error");
            PackageStatus::Unknown(msg.to_string())
        }
        _ => return None,
    };

    Some(CacheEntry { status, timestamp })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_format() {
        let key = RegistryCache::cache_key(RegistryType::PyPI, "requests");
        assert_eq!(key, "pypi:requests");

        let key = RegistryCache::cache_key(RegistryType::Npm, "@types/node");
        assert_eq!(key, "npm:@types/node");
    }

    #[test]
    fn test_format_parse_cache_entry() {
        let entry = CacheEntry {
            status: PackageStatus::Exists,
            timestamp: 1234567890,
        };
        let formatted = format_cache_entry(&entry);
        let parsed = parse_cache_entry(&formatted).unwrap();

        assert_eq!(parsed.status, PackageStatus::Exists);
        assert_eq!(parsed.timestamp, 1234567890);
    }

    #[test]
    fn test_format_parse_notfound() {
        let entry = CacheEntry {
            status: PackageStatus::NotFound,
            timestamp: 1234567890,
        };
        let formatted = format_cache_entry(&entry);
        let parsed = parse_cache_entry(&formatted).unwrap();

        assert_eq!(parsed.status, PackageStatus::NotFound);
    }

    #[test]
    fn test_memory_cache() {
        let cache = RegistryCache::new(24);

        // Set a value
        cache.set(RegistryType::PyPI, "requests", PackageStatus::Exists);

        // Get it back
        let result = cache.get(RegistryType::PyPI, "requests");
        assert_eq!(result, Some(PackageStatus::Exists));

        // Different package should not be cached
        let result = cache.get(RegistryType::PyPI, "flask");
        assert_eq!(result, None);
    }
}
