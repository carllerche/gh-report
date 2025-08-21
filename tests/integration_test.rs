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
    let mut state1 = State::default();
    state1.add_repository("owner/repo1");
    state1.add_repository("owner/repo2");
    state1.save(&state_file)?;

    // Load state
    let state2 = State::load(&state_file)?;

    // Verify state was persisted correctly
    assert_eq!(state2.tracked_repos.len(), 2);
    assert!(state2.tracked_repos.contains_key("owner/repo1"));
    assert!(state2.tracked_repos.contains_key("owner/repo2"));

    Ok(())
}

/// Test configuration loading and validation
#[test]
fn test_config_validation() -> Result<()> {
    let config = Config::default();

    // Verify default values
    assert_eq!(config.settings.max_lookback_days, 30);
    assert_eq!(config.settings.max_issues_per_report, 100);
    assert_eq!(config.settings.inactive_repo_threshold_days, 30);
    assert_eq!(config.claude.primary_model, "sonnet");
    assert_eq!(config.claude.secondary_model, "haiku");
    assert!(config.cache.enabled);
    assert_eq!(config.cache.ttl_hours, 24);

    Ok(())
}

/// Test cache functionality
#[test]
fn test_cache_integration() -> Result<()> {
    use gh_report::cache::CacheManager;

    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().to_path_buf();

    // Create cache manager
    let cache = CacheManager::new(cache_dir, 24, true);
    cache.initialize()?;

    // Test GitHub response caching
    let test_data = b"test github response";
    cache.cache_github_response("test_key", test_data)?;

    let retrieved = cache.get_github_response("test_key")?;
    assert_eq!(retrieved, Some(test_data.to_vec()));

    // Test Claude response caching
    let claude_response = "test claude response";
    cache.cache_claude_response("claude_key", claude_response)?;

    let retrieved_claude = cache.get_claude_response("claude_key")?;
    assert_eq!(retrieved_claude, Some(claude_response.to_string()));

    // Test cache stats
    let stats = cache.get_stats()?;
    assert_eq!(stats.total_entries, 2);
    assert_eq!(stats.github_entries, 1);
    assert_eq!(stats.claude_entries, 1);

    Ok(())
}

/// Test intelligent analysis
#[test]
fn test_intelligent_analysis() -> Result<()> {
    use gh_report::github::{Author, CommentCount, Issue, IssueState, Label, RepoActivity};
    use gh_report::intelligence::IntelligentAnalyzer;
    use jiff::Timestamp;
    use std::collections::BTreeMap;

    let config = Config::default();
    let analyzer = IntelligentAnalyzer::new(&config);

    // Create test data
    let mut activities = BTreeMap::new();
    let mut repo_activity = RepoActivity::default();

    // Add high-priority issue
    repo_activity.new_issues.push(Issue {
        number: 1,
        title: "BREAKING: API change".to_string(),
        body: Some("This is a breaking change".to_string()),
        state: IssueState::Open,
        author: Author {
            login: "user".to_string(),
            user_type: None,
        },
        created_at: Timestamp::now(),
        updated_at: Timestamp::now(),
        labels: vec![Label {
            name: "breaking-change".to_string(),
            color: Some("red".to_string()),
            description: None,
        }],
        url: "https://github.com/test/repo/issues/1".to_string(),
        comments: CommentCount { total_count: 5 },
        is_pull_request: false,
    });

    activities.insert("test/repo".to_string(), repo_activity);

    // Run analysis
    let result = analyzer.analyze(&activities);

    // Verify analysis results
    assert!(!result.prioritized_issues.is_empty());
    assert!(result.prioritized_issues[0].score.total > 0);

    Ok(())
}

/// Test dynamic repository management structures
#[test]
fn test_dynamic_repository_structures() -> Result<()> {
    use gh_report::dynamic::ActivityMetrics;

    // Test activity metrics
    let metrics = ActivityMetrics {
        commits: 10,
        prs: 5,
        issues: 3,
        comments: 20,
    };

    // Verify the structure works
    assert_eq!(metrics.commits, 10);
    assert_eq!(metrics.prs, 5);
    assert_eq!(metrics.issues, 3);
    assert_eq!(metrics.comments, 20);

    Ok(())
}

/// Test report structures
#[test]
fn test_report_structures() -> Result<()> {
    use gh_report::report::Report;
    use jiff::Timestamp;

    let report = Report {
        title: "Test Report".to_string(),
        content: "Test content".to_string(),
        timestamp: Timestamp::now(),
        estimated_cost: 0.05,
    };

    assert_eq!(report.title, "Test Report");
    assert_eq!(report.content, "Test content");
    assert_eq!(report.estimated_cost, 0.05);

    Ok(())
}

/// Test error handling
#[test]
fn test_error_handling() -> Result<()> {
    use anyhow::anyhow;
    use gh_report::error::user_friendly_error;

    // Test GitHub CLI error
    let error = anyhow!("gh: command not found");
    let user_error = user_friendly_error(&error);
    assert_eq!(user_error.message(), "GitHub CLI is not installed");

    // Test API key error
    let error = anyhow!("ANTHROPIC_API_KEY not set");
    let user_error = user_friendly_error(&error);
    assert_eq!(user_error.message(), "Anthropic API key not configured");

    Ok(())
}

/// Test configuration with custom values
#[test]
fn test_config_with_custom_values() -> Result<()> {
    use gh_report::config::{Importance, RepoConfig};

    let mut config = Config::default();

    // Add custom repo
    config.repos.push(RepoConfig {
        name: "test/repo".to_string(),
        labels: vec!["test".to_string()],
        watch_rules: Some(vec!["pattern".to_string()]),
        importance_override: Some(Importance::High),
        custom_context: Some("Custom context".to_string()),
    });

    assert_eq!(config.repos.len(), 1);
    assert_eq!(config.repos[0].name, "test/repo");
    assert_eq!(config.repos[0].importance_override, Some(Importance::High));

    Ok(())
}
