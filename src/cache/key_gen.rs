use sha2::{Digest, Sha256};
use std::fmt::Write;

/// Generate a cache key from components
pub fn generate_cache_key(components: &[&str]) -> String {
    // For simple keys, just join with underscore
    if components.iter().all(|c| is_safe_key_component(c)) {
        return components.join("_");
    }

    // For complex keys, use hash
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update(component.as_bytes());
        hasher.update(b"\0"); // Separator to prevent collisions
    }

    let result = hasher.finalize();
    hex_string(&result[..8]) // Use first 8 bytes for shorter keys
}

/// Check if a string is safe to use directly in a cache key
fn is_safe_key_component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Convert bytes to hex string
fn hex_string(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hex, "{:02x}", byte).unwrap();
    }
    hex
}

/// Builder for complex cache keys
pub struct CacheKeyBuilder {
    components: Vec<String>,
}

impl CacheKeyBuilder {
    /// Create new cache key builder
    pub fn new() -> Self {
        CacheKeyBuilder {
            components: Vec::new(),
        }
    }

    /// Add a component to the key
    pub fn add(mut self, component: impl Into<String>) -> Self {
        self.components.push(component.into());
        self
    }

    /// Add an optional component
    pub fn add_opt(mut self, component: Option<impl Into<String>>) -> Self {
        if let Some(c) = component {
            self.components.push(c.into());
        }
        self
    }

    /// Add a namespace prefix
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.components.insert(0, namespace.to_string());
        self
    }

    /// Add a timestamp component
    pub fn with_timestamp(mut self, timestamp: jiff::Timestamp) -> Self {
        self.components.push(timestamp.as_millisecond().to_string());
        self
    }

    /// Build the final cache key
    pub fn build(self) -> String {
        let refs: Vec<&str> = self.components.iter().map(|s| s.as_str()).collect();
        generate_cache_key(&refs)
    }
}

impl Default for CacheKeyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_cache_key_simple() {
        let key = generate_cache_key(&["repo", "issue", "123"]);
        assert_eq!(key, "repo_issue_123");
    }

    #[test]
    fn test_generate_cache_key_complex() {
        let key = generate_cache_key(&["repo/with/slashes", "issue#123"]);
        // Should produce a hash
        assert!(key.len() == 16); // 8 bytes as hex
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cache_key_builder() {
        let key = CacheKeyBuilder::new()
            .with_namespace("test")
            .add("component1")
            .add("component2")
            .build();

        assert_eq!(key, "test_component1_component2");
    }

    #[test]
    fn test_cache_key_builder_optional() {
        let key = CacheKeyBuilder::new()
            .add("base")
            .add_opt(Some("present"))
            .add_opt(None::<String>)
            .build();

        assert_eq!(key, "base_present");
    }

    #[test]
    fn test_is_safe_key_component() {
        assert!(is_safe_key_component("simple"));
        assert!(is_safe_key_component("with-dash"));
        assert!(is_safe_key_component("with_underscore"));
        assert!(is_safe_key_component("123numbers"));

        assert!(!is_safe_key_component(""));
        assert!(!is_safe_key_component("with/slash"));
        assert!(!is_safe_key_component("with space"));
        assert!(!is_safe_key_component("special@char"));

        let long = "a".repeat(65);
        assert!(!is_safe_key_component(&long));
    }
}
