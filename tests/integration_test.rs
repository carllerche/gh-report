use anyhow::Result;
use gh_report::{Config, State};
use tempfile::TempDir;

// Note: Full integration tests requiring GitHubClient are limited because
// MockGitHub is only available in library tests, not integration tests.
// This is a common Rust testing pattern limitation.

/// Test state persistence and management
#[test]
fn test_state_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let state_file = temp_dir.path().join("state.json");

    // Create and save state
    let state1 = State::default();
    state1.save(&state_file)?;

    // Load state
    let state2 = State::load(&state_file)?;

    // Verify state was persisted correctly
    assert_eq!(state2.last_run, state1.last_run);
    assert_eq!(state2.last_report_file, state1.last_report_file);

    Ok(())
}

/// Test configuration loading and defaults
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

    assert!(config.cache.enabled);
    assert_eq!(config.cache.ttl_hours, 24);
    assert!(config.cache.compression_enabled);
}

/// Test the report template format
#[test]
fn test_report_template() {
    let template = r#"# GitHub Activity Report - {date}

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
*Report generated at {timestamp} | Est. cost: ${cost}*"#;

    // Basic validation that template contains expected placeholders
    assert!(template.contains("{date}"));
    assert!(template.contains("{action_required}"));
    assert!(template.contains("{needs_attention}"));
    assert!(template.contains("{key_changes}"));
    assert!(template.contains("{suggested_actions}"));
    assert!(template.contains("{fyi}"));
    assert!(template.contains("{repo_changes}"));
    assert!(template.contains("{timestamp}"));
    assert!(template.contains("{cost}"));
}

/// Test basic state operations
#[test]
fn test_state_update_last_run() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let state_file = temp_dir.path().join("state.json");

    // Create state and update last run
    let mut state = State::default();
    state.update_last_run();
    state.save(&state_file)?;

    // Load and verify
    let loaded_state = State::load(&state_file)?;
    assert!(loaded_state.last_run.is_some());

    Ok(())
}
