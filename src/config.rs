use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub settings: Settings,
    pub claude: ClaudeConfig,
    #[serde(default)]
    pub report: ReportConfig,
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub report_dir: PathBuf,
    #[serde(default = "default_state_file")]
    pub state_file: PathBuf,
    #[serde(default = "default_file_name_format")]
    pub file_name_format: String,
    #[serde(default = "default_max_lookback_days")]
    pub max_lookback_days: u32,
    #[serde(default = "default_max_issues")]
    pub max_issues_per_report: usize,
    #[serde(default = "default_max_comments")]
    pub max_comments_per_report: usize,
    #[serde(default = "default_inactive_threshold")]
    pub inactive_repo_threshold_days: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClaudeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_helper: Option<String>,
    #[serde(default = "default_primary_model")]
    pub primary_model: String,
    #[serde(default = "default_secondary_model")]
    pub secondary_model: String,
    #[serde(default = "default_cache_responses")]
    pub cache_responses: bool,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_hours: u32,
    #[serde(default = "default_claude_backend")]
    pub backend: ClaudeBackend,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReportConfig {
    #[serde(default = "default_template")]
    pub template: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
    #[serde(default = "default_cache_ttl")]
    pub ttl_hours: u32,
    #[serde(default = "default_compression_enabled")]
    pub compression_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Importance {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClaudeBackend {
    Api,
    Cli,
    Auto, // Try CLI first, fall back to API
}

impl Config {
    /// Load configuration from the default location or a specified path
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => Self::default_config_path()?,
        };

        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;

        let mut config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config from {:?}", config_path))?;

        // Expand home directory in paths
        config.settings.report_dir = expand_tilde(&config.settings.report_dir)?;
        config.settings.state_file = expand_tilde(&config.settings.state_file)?;

        Ok(config)
    }

    /// Get the default configuration file path
    pub fn default_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".config").join("gh-report").join("config.toml"))
    }

    /// Create a default configuration
    pub fn default() -> Self {
        Config {
            settings: Settings {
                report_dir: PathBuf::from("~/Github Reports"),
                state_file: default_state_file(),
                file_name_format: default_file_name_format(),
                max_lookback_days: default_max_lookback_days(),
                max_issues_per_report: default_max_issues(),
                max_comments_per_report: default_max_comments(),
                inactive_repo_threshold_days: default_inactive_threshold(),
            },
            claude: ClaudeConfig {
                api_key: None,
                api_key_helper: None,
                primary_model: default_primary_model(),
                secondary_model: default_secondary_model(),
                cache_responses: default_cache_responses(),
                cache_ttl_hours: default_cache_ttl(),
                backend: default_claude_backend(),
            },
            report: ReportConfig {
                template: default_template(),
            },
            cache: CacheConfig {
                enabled: default_cache_enabled(),
                ttl_hours: default_cache_ttl(),
                compression_enabled: default_compression_enabled(),
                cache_dir: None,
            },
        }
    }
}

/// Expand tilde in paths to home directory
fn expand_tilde(path: &Path) -> Result<PathBuf> {
    if let Some(s) = path.to_str() {
        if s.starts_with("~/") {
            let home = dirs::home_dir().context("Could not determine home directory")?;
            return Ok(home.join(&s[2..]));
        }
    }
    Ok(path.to_path_buf())
}

// Default value functions
fn default_state_file() -> PathBuf {
    PathBuf::from("~/Github Reports/.gh-report-state.json")
}

fn default_file_name_format() -> String {
    "{yyyy-mm-dd} - Github - {short-title}".to_string()
}

fn default_max_lookback_days() -> u32 {
    30
}

fn default_max_issues() -> usize {
    100
}

fn default_max_comments() -> usize {
    500
}

fn default_inactive_threshold() -> u32 {
    30
}

fn default_primary_model() -> String {
    "sonnet".to_string()
}

fn default_secondary_model() -> String {
    "haiku".to_string()
}

fn default_cache_responses() -> bool {
    true
}

fn default_cache_ttl() -> u32 {
    24
}

fn default_template() -> String {
    r#"# GitHub Activity Report - {date}

## 🚨 Action Required
{action_required}

## 👀 Needs Attention
{needs_attention}

## 📋 Key Changes and Proposals
{key_changes}

## 💡 Suggested Actions
{suggested_actions}

## 📰 FYI
{fyi}

## 📊 Repository Activity Changes
{repo_changes}

---
*Report generated at {timestamp} | Est. cost: ${cost}*"#
        .to_string()
}

fn default_cache_enabled() -> bool {
    true
}

fn default_compression_enabled() -> bool {
    true
}

fn default_claude_backend() -> ClaudeBackend {
    ClaudeBackend::Auto
}

// Default implementation for ReportConfig
impl Default for ReportConfig {
    fn default() -> Self {
        ReportConfig {
            template: default_template(),
        }
    }
}

// Default implementation for CacheConfig
impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig {
            enabled: default_cache_enabled(),
            ttl_hours: default_cache_ttl(),
            compression_enabled: default_compression_enabled(),
            cache_dir: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();

        assert_eq!(config.settings.max_lookback_days, 30);
        assert_eq!(config.settings.max_issues_per_report, 100);
        assert_eq!(config.settings.max_comments_per_report, 500);
        assert_eq!(config.settings.inactive_repo_threshold_days, 30);

        assert_eq!(config.claude.primary_model, "sonnet");
        assert_eq!(config.claude.secondary_model, "haiku");
        assert!(config.claude.cache_responses);
        assert_eq!(config.claude.cache_ttl_hours, 24);

        // Dynamic repos have been removed

        assert!(config.cache.enabled);
        assert_eq!(config.cache.ttl_hours, 24);
        assert!(config.cache.compression_enabled);
    }

    #[test]
    fn test_path_expansion() {
        let home = dirs::home_dir().unwrap();
        let path = PathBuf::from("~/test/path");
        let expanded = expand_tilde(&path).unwrap();

        assert_eq!(expanded, home.join("test/path"));

        // Test path without tilde
        let absolute_path = PathBuf::from("/absolute/path");
        let expanded = expand_tilde(&absolute_path).unwrap();
        assert_eq!(expanded, absolute_path);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();

        // Serialize to TOML
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("[settings]"));
        assert!(toml_str.contains("[claude]"));

        // Deserialize back
        let config2: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            config.settings.max_lookback_days,
            config2.settings.max_lookback_days
        );
    }

    #[test]
    fn test_importance_ordering() {
        use Importance::*;

        assert!(Low < Medium);
        assert!(Medium < High);
        assert!(High < Critical);

        let mut importances = vec![Critical, Low, High, Medium];
        importances.sort();
        assert_eq!(importances, vec![Low, Medium, High, Critical]);
    }

    #[test]
    fn test_default_config_path() {
        let path = Config::default_config_path().unwrap();
        let path_str = path.to_string_lossy();
        
        // Should always use ~/.config/gh-report/config.toml on all platforms
        assert!(path_str.ends_with(".config/gh-report/config.toml"));
        assert!(path_str.contains(".config"));
    }
}
