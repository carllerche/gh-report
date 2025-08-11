use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub settings: Settings,
    pub claude: ClaudeConfig,
    #[serde(default)]
    pub report: ReportConfig,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub repos: Vec<RepoConfig>,
    #[serde(default)]
    pub dynamic_repos: DynamicReposConfig,
    #[serde(default)]
    pub watch_rules: HashMap<String, Vec<String>>,
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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReportConfig {
    #[serde(default = "default_template")]
    pub template: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Label {
    pub name: String,
    pub description: String,
    pub watch_rules: Vec<String>,
    pub importance: Importance,
    pub context: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RepoConfig {
    pub name: String,
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_rules: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance_override: Option<Importance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_context: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DynamicReposConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_auto_add_threshold")]
    pub auto_add_threshold_days: u32,
    #[serde(default = "default_auto_remove_threshold")]
    pub auto_remove_threshold_days: u32,
    #[serde(default = "default_activity_weights")]
    pub activity_weights: ActivityWeights,
    #[serde(default = "default_min_score")]
    pub min_activity_score: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActivityWeights {
    pub commits: u32,
    pub prs: u32,
    pub issues: u32,
    pub comments: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Importance {
    Low,
    Medium,
    High,
    Critical,
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
        Ok(home.join(".gh-report.toml"))
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
            },
            report: ReportConfig {
                template: default_template(),
            },
            labels: vec![],
            repos: vec![],
            dynamic_repos: DynamicReposConfig {
                enabled: true,
                auto_add_threshold_days: default_auto_add_threshold(),
                auto_remove_threshold_days: default_auto_remove_threshold(),
                activity_weights: default_activity_weights(),
                min_activity_score: default_min_score(),
            },
            watch_rules: default_watch_rules(),
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

fn default_true() -> bool {
    true
}

fn default_auto_add_threshold() -> u32 {
    7
}

fn default_auto_remove_threshold() -> u32 {
    30
}

fn default_activity_weights() -> ActivityWeights {
    ActivityWeights {
        commits: 4,
        prs: 3,
        issues: 2,
        comments: 1,
    }
}

fn default_min_score() -> u32 {
    5
}

fn default_watch_rules() -> HashMap<String, Vec<String>> {
    let mut rules = HashMap::new();
    rules.insert(
        "api_changes".to_string(),
        vec![
            "public API".to_string(),
            "breaking change".to_string(),
            "deprecation".to_string(),
            "new feature".to_string(),
        ],
    );
    rules.insert(
        "breaking_changes".to_string(),
        vec![
            "BREAKING".to_string(),
            "migration".to_string(),
            "major version".to_string(),
        ],
    );
    rules.insert(
        "security_issues".to_string(),
        vec![
            "security".to_string(),
            "vulnerability".to_string(),
            "CVE".to_string(),
            "exploit".to_string(),
        ],
    );
    rules.insert(
        "performance".to_string(),
        vec![
            "performance".to_string(),
            "regression".to_string(),
            "benchmark".to_string(),
            "slow".to_string(),
        ],
    );
    rules.insert("mentions".to_string(), vec!["@{username}".to_string()]);
    rules.insert(
        "review_requests".to_string(),
        vec![
            "review requested".to_string(),
            "PTAL".to_string(),
            "feedback needed".to_string(),
        ],
    );
    rules.insert("all_activity".to_string(), vec![]);
    rules
}

// Default implementation for ReportConfig
impl Default for ReportConfig {
    fn default() -> Self {
        ReportConfig {
            template: default_template(),
        }
    }
}

// Default implementation for DynamicReposConfig
impl Default for DynamicReposConfig {
    fn default() -> Self {
        DynamicReposConfig {
            enabled: true,
            auto_add_threshold_days: default_auto_add_threshold(),
            auto_remove_threshold_days: default_auto_remove_threshold(),
            activity_weights: default_activity_weights(),
            min_activity_score: default_min_score(),
        }
    }
}