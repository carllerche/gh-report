use anyhow::{Context, Result};
use clap::Parser;
use gh_report::{
    cli::{Cli, Commands},
    dynamic::DynamicRepoManager,
    github::GitHubClient,
    report::ReportGenerator,
    summarize::IssueSummarizer,
    Config, State,
};
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging based on verbosity
    setup_logging(cli.verbose)?;

    // Run the appropriate command
    match cli.command {
        Some(Commands::Init { lookback, output }) => {
            info!("Initializing configuration based on GitHub activity");
            init_command(lookback, output)?;
        }
        Some(Commands::RebuildState) => {
            info!("Rebuilding state from existing reports");
            rebuild_state_command(&cli)?;
        }
        Some(Commands::Summarize {
            ref target,
            ref output,
            no_recommendations,
        }) => {
            info!("Summarizing issue/PR: {}", target);
            summarize_command(target, output.as_deref(), no_recommendations, &cli)?;
        }
        Some(Commands::ListRepos { lookback }) => {
            info!("Listing repositories with recent activity");
            list_repos_command(lookback, &cli)?;
        }
        Some(Commands::Activity { days, ref include_types, ref exclude_types }) => {
            info!("Showing GitHub activity feed");
            activity_command(days, include_types.as_ref(), exclude_types.as_ref(), &cli)?;
        }
        None => {
            // Main report generation command
            generate_report(&cli)?;
        }
    }

    Ok(())
}

fn setup_logging(verbosity: u8) -> Result<()> {
    let filter = match verbosity {
        0 => EnvFilter::new("warn"),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    Ok(())
}

fn generate_report(cli: &Cli) -> Result<()> {
    // Check GitHub CLI first
    info!("Checking GitHub CLI");
    match gh_report::github::check_gh_version() {
        Ok(version) => info!("Using gh version {}", version),
        Err(e) => {
            error!("GitHub CLI check failed: {}", e);
            println!("❌ {}", e);
            println!("\nPlease install GitHub CLI from: https://cli.github.com/");
            return Err(e);
        }
    }

    info!("Loading configuration");
    let mut config = Config::load(cli.config.as_deref()).context("Failed to load configuration")?;

    // Override Claude backend if specified
    if let Some(backend_str) = &cli.claude_backend {
        use gh_report::config::ClaudeBackend;
        let backend = match backend_str.to_lowercase().as_str() {
            "api" => ClaudeBackend::Api,
            "cli" => ClaudeBackend::Cli,
            "auto" => ClaudeBackend::Auto,
            _ => {
                error!("Invalid Claude backend: {}", backend_str);
                println!(
                    "❌ Invalid Claude backend: '{}'. Valid options: api, cli, auto",
                    backend_str
                );
                std::process::exit(1);
            }
        };
        info!("Using Claude backend override: {:?}", backend);
        config.claude.backend = backend;
    }

    // Override report directory if specified
    if let Some(report_dir) = &cli.report_dir {
        info!("Using custom report directory: {:?}", report_dir);
        config.settings.report_dir = report_dir.clone();

        // Create the report directory if it doesn't exist
        std::fs::create_dir_all(report_dir)
            .with_context(|| format!("Failed to create report directory: {:?}", report_dir))?;
    }

    // Override state file location if specified
    let state_file = if let Some(state_path) = &cli.state {
        info!("Using custom state file: {:?}", state_path);

        // Create parent directory if needed
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create state directory: {:?}", parent))?;
        }

        state_path.clone()
    } else {
        config.settings.state_file.clone()
    };

    info!("Loading state");
    let mut state = State::load(&state_file).context("Failed to load state")?;

    // Handle cache operations
    if cli.clear_cache {
        info!("Clearing cache");
        clear_cache(&config)?;
    }

    // Create GitHub client for dynamic updates
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Update dynamic repositories if enabled
    if config.dynamic_repos.enabled {
        println!("🔍 Discovering active repositories...");
        info!("Updating dynamic repository list");
        let mut manager = DynamicRepoManager::new(&config, &mut state, &github_client);
        match manager.update_repositories() {
            Ok(result) => {
                if !result.added.is_empty() {
                    println!("➕ Added {} new repositories", result.added.len());
                    for repo in result.added.iter().take(5) {
                        println!("   - {}", repo);
                    }
                    if result.added.len() > 5 {
                        println!("   ... and {} more", result.added.len() - 5);
                    }
                }
                if !result.removed.is_empty() {
                    println!("➖ Removed {} inactive repositories", result.removed.len());
                }
                println!("📚 Tracking {} repositories total", result.total_tracked);
                info!(
                    "Repository update: {} added, {} removed, {} tracked total",
                    result.added.len(),
                    result.removed.len(),
                    result.total_tracked
                );
            }
            Err(e) => {
                warn!("Failed to update dynamic repositories: {}", e);
                println!("⚠️  Failed to discover repositories: {}", e);
                println!("    You may need to manually add repositories to the config file");
            }
        }
    } else {
        println!(
            "📚 Tracking {} configured repositories",
            state.tracked_repos.len()
        );
    }

    // Dry run is now handled in the report generator

    if cli.estimate_cost {
        info!("Estimating Claude API costs");
        estimate_costs(&config, &state)?;
        return Ok(());
    }

    // Determine the lookback days
    let lookback_days = if cli.week {
        info!("Generating weekly report (7 days)");
        7
    } else if let Some(since_str) = &cli.since {
        info!("Using custom since date: {}", since_str);
        // For now, parse simple day count like "7d" or just a number
        if since_str.ends_with('d') {
            let days_str = &since_str[..since_str.len() - 1];
            days_str
                .parse::<u32>()
                .with_context(|| format!("Invalid since format: {}", since_str))?
        } else {
            since_str
                .parse::<u32>()
                .with_context(|| format!("Invalid since format: {}", since_str))?
        }
    } else {
        // Calculate days since last run, or use max lookback
        if let Some(last_run) = state.last_run {
            let now = jiff::Timestamp::now();
            let diff = now - last_run;
            let days = (diff.get_days() as u32).max(1);
            days.min(config.settings.max_lookback_days)
        } else {
            config.settings.max_lookback_days
        }
    };

    info!("Generating report for the last {} days", lookback_days);
    println!("✓ Loading configuration");
    if let Some(last_run) = state.last_run {
        println!("✓ Last report: {}", last_run.strftime("%Y-%m-%d %H:%M"));
    } else {
        println!("✓ First run - no previous report found");
    }

    // Generate the report
    println!("📊 Fetching GitHub activity...");

    // Check if AI summarization is available
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        println!("ℹ️  Running without AI summarization (ANTHROPIC_API_KEY not set)");
    }

    let generator = ReportGenerator::new(github_client, &config, &state);
    let report = generator
        .generate(lookback_days)
        .context("Failed to generate report")?;

    // Save the report
    let report_path = report.save(&config).context("Failed to save report")?;

    println!("✓ Report saved to: {:?}", report_path);

    // Update state
    state.update_last_run();
    state.save(&state_file).context("Failed to save state")?;

    Ok(())
}

fn init_command(lookback: u32, output: Option<PathBuf>) -> Result<()> {
    let config_path = output
        .unwrap_or_else(|| Config::default_config_path().expect("Could not determine config path"));

    if config_path.exists() {
        warn!("Configuration already exists at {:?}", config_path);
        println!("Configuration file already exists at: {:?}", config_path);
        println!("Please remove it first if you want to regenerate.");
        return Ok(());
    }

    println!(
        "Analyzing GitHub activity for the past {} days...",
        lookback
    );

    // Check GitHub CLI first
    match gh_report::github::check_gh_version() {
        Ok(version) => info!("Using gh version {}", version),
        Err(e) => {
            error!("GitHub CLI check failed: {}", e);
            println!("❌ {}", e);
            println!("\nPlease install GitHub CLI from: https://cli.github.com/");
            return Err(e);
        }
    }

    // Create GitHub client
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Create default config and state for discovery
    let mut config = Config::default();
    let mut state = State::default();

    // Use dynamic repo manager to discover repositories
    let mut manager = DynamicRepoManager::new(&config, &mut state, &github_client);
    let init_result = manager
        .initialize_repositories(lookback)
        .context("Failed to discover repositories")?;

    println!(
        "✓ Found {} repositories with recent activity",
        init_result.total_found
    );

    if init_result.repositories.is_empty() {
        println!("\n⚠️  No repositories found with recent activity.");
        println!("Creating default configuration without repositories.");
    } else {
        println!("\nTop repositories by activity score:");
        for (repo, score) in init_result.repositories.iter().take(10) {
            println!("  - {} (score: {})", repo, score);
        }

        if init_result.repositories.len() > 10 {
            println!("  ... and {} more", init_result.repositories.len() - 10);
        }

        // Add discovered repos to config
        for (repo_name, _score) in &init_result.repositories {
            config.repos.push(gh_report::config::RepoConfig {
                name: repo_name.clone(),
                labels: vec![],
                watch_rules: None,
                importance_override: None,
                custom_context: None,
            });
        }
    }

    // Write configuration
    let config_str = toml::to_string_pretty(&config).context("Failed to serialize config")?;

    std::fs::write(&config_path, config_str)
        .with_context(|| format!("Failed to write config to {:?}", config_path))?;

    println!("\n✓ Configuration created at: {:?}", config_path);

    // Also save initial state
    let state_path = config.settings.state_file.clone();
    let expanded_state_path = if let Some(s) = state_path.to_str() {
        if s.starts_with("~/") {
            let home = dirs::home_dir().context("Could not determine home directory")?;
            home.join(&s[2..])
        } else {
            state_path
        }
    } else {
        state_path
    };

    // Create state directory if needed
    if let Some(parent) = expanded_state_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create state directory")?;
    }

    state
        .save(&expanded_state_path)
        .context("Failed to save initial state")?;

    println!("✓ Initial state saved");

    println!("\nNext steps:");
    println!("1. Set your Anthropic API key:");
    println!("   export ANTHROPIC_API_KEY='your-key-here'");
    println!("2. Review and customize the configuration file");
    println!("3. Run 'gh-report' to generate your first report");

    Ok(())
}

fn rebuild_state_command(cli: &Cli) -> Result<()> {
    let config = Config::load(cli.config.as_deref()).context("Failed to load configuration")?;

    println!(
        "Scanning report directory: {:?}",
        config.settings.report_dir
    );

    // TODO: Implement state rebuilding from reports
    println!("⚠️  State rebuilding not yet implemented");
    println!("This will scan existing reports and rebuild the state file.");

    Ok(())
}

fn clear_cache(config: &Config) -> Result<()> {
    let cache_dir = config.settings.report_dir.join(".cache");
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir)
            .with_context(|| format!("Failed to clear cache at {:?}", cache_dir))?;
        info!("Cache cleared");
    } else {
        info!("No cache to clear");
    }
    Ok(())
}

fn summarize_command(
    target: &str,
    output_path: Option<&Path>,
    no_recommendations: bool,
    cli: &Cli,
) -> Result<()> {
    // Check GitHub CLI first
    match gh_report::github::check_gh_version() {
        Ok(version) => info!("Using gh version {}", version),
        Err(e) => {
            error!("GitHub CLI check failed: {}", e);
            println!("❌ {}", e);
            println!("\nPlease install GitHub CLI from: https://cli.github.com/");
            return Err(e);
        }
    }

    // Load configuration
    let config = Config::load(cli.config.as_deref())?;

    // Create GitHub client
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Create summarizer
    let summarizer = IssueSummarizer::new(github_client, &config);

    // Generate summary
    let include_recommendations = !no_recommendations;
    match summarizer.summarize(target, output_path, include_recommendations) {
        Ok(output_file) => {
            println!("✓ Summary saved to: {}", output_file);
            Ok(())
        }
        Err(e) => {
            error!("Failed to generate summary: {}", e);
            println!("❌ {}", e);
            Err(e)
        }
    }
}

fn estimate_costs(config: &Config, state: &State) -> Result<()> {
    // TODO: Implement actual cost estimation based on data volume
    println!("Estimating costs based on current configuration...");
    println!("\nRepositories to check: {}", state.tracked_repos.len());
    println!(
        "Max issues per report: {}",
        config.settings.max_issues_per_report
    );
    println!(
        "Max comments per report: {}",
        config.settings.max_comments_per_report
    );

    println!("\nEstimated Claude API usage:");
    println!(
        "  Primary model ({}): ~5000 tokens",
        config.claude.primary_model
    );
    println!(
        "  Secondary model ({}): ~2000 tokens",
        config.claude.secondary_model
    );
    println!("\nEstimated cost: $0.02-0.04");

    Ok(())
}

fn list_repos_command(lookback: u32, _cli: &Cli) -> Result<()> {
    // Check GitHub CLI first
    match gh_report::github::check_gh_version() {
        Ok(version) => info!("Using gh version {}", version),
        Err(e) => {
            error!("GitHub CLI check failed: {}", e);
            println!("❌ {}", e);
            println!("\nPlease install GitHub CLI from: https://cli.github.com/");
            return Err(e);
        }
    }

    println!(
        "Discovering repositories you have write access to with recent activity (last 30 days)...",
    );

    // Create GitHub client
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Create default config and state for discovery
    let config = Config::default();
    let mut state = State::default();

    // Use dynamic repo manager to discover repositories
    let mut manager = DynamicRepoManager::new(&config, &mut state, &github_client);
    let init_result = manager
        .initialize_repositories(lookback)
        .context("Failed to discover repositories")?;

    if init_result.repositories.is_empty() {
        println!("\nNo repositories found matching criteria.");
        println!("\nThis means either:");
        println!(
            "  - No repositories you have write access to have recent activity (last 30 days)"
        );
        println!("  - You have recent activity but no write permissions to those repositories");
        println!("\nTroubleshooting:");
        println!("  - Check your GitHub CLI authentication: gh auth status");
        println!("  - Verify you have recent GitHub activity (commits, PRs, comments)");
        println!("  - Ensure you have push/write access to repositories you expect to see");
        return Ok(());
    }

    // Group repositories by organization
    use std::collections::BTreeMap;
    let mut grouped_repos: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();

    for (repo_name, score) in &init_result.repositories {
        let (org, repo) = if let Some(slash_pos) = repo_name.find('/') {
            let org = &repo_name[..slash_pos];
            let repo = &repo_name[slash_pos + 1..];
            (org.to_string(), repo.to_string())
        } else {
            // Handle edge case of no organization
            ("(no org)".to_string(), repo_name.clone())
        };

        grouped_repos
            .entry(org)
            .or_insert_with(Vec::new)
            .push((repo, *score));
    }

    // Sort repositories within each org by score (highest first)
    for repos in grouped_repos.values_mut() {
        repos.sort_by(|a, b| b.1.cmp(&a.1));
    }

    println!(
        "\nFound {} repositories across {} organizations:",
        init_result.repositories.len(),
        grouped_repos.len()
    );
    println!("\n{}", "=".repeat(60));

    for (org, repos) in &grouped_repos {
        println!("\n**{}** ({} repositories)", org, repos.len());

        for (repo, _score) in repos {
            println!("   {}", repo);
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("\nTo initialize with these repositories:");
    println!("   gh-report init --lookback {}", lookback);
    println!("\nSelection criteria:");
    println!("   - Must have write/push access to the repository");
    println!("   - Must have recent activity (last 30 days): commits, PRs, comments, etc.");

    Ok(())
}

fn filter_events<'a>(
    events: &'a [gh_report::github::ActivityEvent], 
    include_types: Option<&Vec<String>>, 
    exclude_types: Option<&Vec<String>>
) -> Vec<&'a gh_report::github::ActivityEvent> {
    let default_included_types = vec![
        "IssueCommentEvent".to_string(),
        "PullRequestEvent".to_string(),
        "IssuesEvent".to_string(),
        "PullRequestReviewCommentEvent".to_string(),
        "PullRequestReviewEvent".to_string(),
    ];
    
    events.iter().filter(|event| {
        // First check include types (default to user's preferred list if not specified)
        let included_types = include_types.unwrap_or(&default_included_types);
        if !included_types.contains(&event.event_type) {
            return false;
        }
        
        // Check exclude types
        if let Some(excluded) = exclude_types {
            if excluded.contains(&event.event_type) {
                return false;
            }
        }
        
        // Special filtering for IssuesEvent - exclude 'labeled' actions
        if event.event_type == "IssuesEvent" {
            if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                if action == "labeled" || action == "unlabeled" {
                    return false;
                }
            }
        }
        
        true
    }).collect()
}

fn activity_command(
    days: u32, 
    include_types: Option<&Vec<String>>, 
    exclude_types: Option<&Vec<String>>, 
    _cli: &Cli
) -> Result<()> {
    // Check GitHub CLI first
    match gh_report::github::check_gh_version() {
        Ok(version) => info!("Using gh version {}", version),
        Err(e) => {
            error!("GitHub CLI check failed: {}", e);
            println!("❌ {}", e);
            println!("\nPlease install GitHub CLI from: https://cli.github.com/");
            return Err(e);
        }
    }

    println!("Fetching activity on repositories you're subscribed to for the last {} days...", days);

    // Create GitHub client
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Fetch activity events
    let all_events = github_client
        .fetch_activity(days)
        .context("Failed to fetch activity")?;

    // Apply event type filtering
    let events = filter_events(&all_events, include_types, exclude_types);

    if events.is_empty() {
        println!("\nNo matching activity found in the last {} days.", days);
        if all_events.len() > 0 {
            println!("({} events were filtered out)", all_events.len());
        }
        return Ok(());
    }

    // Group events by date → repo → issue/PR
    use std::collections::BTreeMap;
    
    let mut events_by_date: BTreeMap<String, BTreeMap<String, BTreeMap<Option<IssueKey>, Vec<&gh_report::github::ActivityEvent>>>> = BTreeMap::new();

    for event in &events {
        let date_key = event.created_at.strftime("%Y-%m-%d").to_string();
        let repo_name = event.repo.name.clone();
        
        // Extract issue/PR number if available
        let issue_key = extract_issue_key(event);
        
        events_by_date
            .entry(date_key)
            .or_insert_with(BTreeMap::new)
            .entry(repo_name)
            .or_insert_with(BTreeMap::new)
            .entry(issue_key)
            .or_insert_with(Vec::new)
            .push(event);
    }

    println!("\nActivity Summary ({} events):", events.len());
    println!("{}", "=".repeat(60));

    // Display events grouped by date → repo → issue/PR
    for (date, repos_events) in events_by_date.iter().rev() {
        let total_events: usize = repos_events.values()
            .map(|repo_issues| repo_issues.values().map(|events| events.len()).sum::<usize>())
            .sum();
        println!("\n**{}** ({} events)", date, total_events);
        
        for (repo_name, issues_events) in repos_events {
            let _repo_event_count: usize = issues_events.values().map(|events| events.len()).sum();
            println!("  {}", repo_name);
            
            for (issue_key, issue_events) in issues_events {
                match issue_key {
                    Some(key) => {
                        let item_type = if key.is_pr { "PR" } else { "Issue" };
                        let actors: Vec<String> = issue_events.iter()
                            .map(|e| format!("@{}", e.actor.login))
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();
                        let actions = consolidate_actions(issue_events);
                        println!("    {} #{}: {} ({})", item_type, key.issue_number, actions, actors.join(", "));
                    }
                    None => {
                        // Events without specific issue/PR (e.g., general repo activity)
                        for event in issue_events {
                            let event_desc = format_activity_event(event);
                            println!("    {}", event_desc);
                        }
                    }
                }
            }
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("\nEvent types found:");
    
    // Count event types
    let mut event_type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for event in &events {
        *event_type_counts.entry(event.event_type.clone()).or_insert(0) += 1;
    }

    let mut sorted_types: Vec<_> = event_type_counts.iter().collect();
    sorted_types.sort_by(|a, b| b.1.cmp(a.1));

    for (event_type, count) in sorted_types {
        println!("   - {}: {}", event_type, count);
    }

    Ok(())
}

fn extract_issue_key(event: &gh_report::github::ActivityEvent) -> Option<IssueKey> {
    match event.event_type.as_str() {
        "PullRequestEvent" => {
            if let Some(pr_number) = event.payload
                .get("pull_request")
                .and_then(|pr| pr.get("number"))
                .and_then(|n| n.as_u64())
            {
                Some(IssueKey {
                    issue_number: pr_number,
                    is_pr: true,
                })
            } else {
                None
            }
        }
        "IssuesEvent" | "IssueCommentEvent" => {
            if let Some(issue_number) = event.payload
                .get("issue")
                .and_then(|issue| issue.get("number"))
                .and_then(|n| n.as_u64())
            {
                // Check if this is actually a PR (issues API includes PRs)
                let is_pr = event.payload
                    .get("issue")
                    .and_then(|issue| issue.get("pull_request"))
                    .is_some();
                
                Some(IssueKey {
                    issue_number,
                    is_pr,
                })
            } else {
                None
            }
        }
        "PullRequestReviewCommentEvent" => {
            if let Some(pr_number) = event.payload
                .get("pull_request")
                .and_then(|pr| pr.get("number"))
                .and_then(|n| n.as_u64())
            {
                Some(IssueKey {
                    issue_number: pr_number,
                    is_pr: true,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn consolidate_actions(events: &[&gh_report::github::ActivityEvent]) -> String {
    use std::collections::HashSet;
    
    let mut actions = HashSet::new();
    
    for event in events {
        match event.event_type.as_str() {
            "PullRequestEvent" => {
                if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                    actions.insert(format!("{} PR", action));
                }
            }
            "IssuesEvent" => {
                if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                    actions.insert(format!("{} issue", action));
                }
            }
            "IssueCommentEvent" => {
                actions.insert("commented".to_string());
            }
            "PullRequestReviewCommentEvent" => {
                actions.insert("reviewed".to_string());
            }
            _ => {
                actions.insert("activity".to_string());
            }
        }
    }
    
    let mut action_list: Vec<String> = actions.into_iter().collect();
    action_list.sort();
    action_list.join(", ")
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct IssueKey {
    issue_number: u64,
    is_pr: bool,
}

fn format_activity_event(event: &gh_report::github::ActivityEvent) -> String {
    let actor = &event.actor.login;

    match event.event_type.as_str() {
        "PushEvent" => {
            if let Some(commits) = event.payload.get("commits").and_then(|c| c.as_array()) {
                format!("@{} pushed {} commit(s)", actor, commits.len())
            } else {
                format!("@{} pushed commits", actor)
            }
        }
        "PullRequestEvent" => {
            if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                if let Some(pr_number) = event.payload
                    .get("pull_request")
                    .and_then(|pr| pr.get("number"))
                    .and_then(|n| n.as_u64())
                {
                    format!("@{} {} PR #{}", actor, action, pr_number)
                } else {
                    format!("@{} {} pull request", actor, action)
                }
            } else {
                format!("@{} pull request activity", actor)
            }
        }
        "IssuesEvent" => {
            if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                if let Some(issue_number) = event.payload
                    .get("issue")
                    .and_then(|issue| issue.get("number"))
                    .and_then(|n| n.as_u64())
                {
                    format!("@{} {} issue #{}", actor, action, issue_number)
                } else {
                    format!("@{} {} issue", actor, action)
                }
            } else {
                format!("@{} issue activity", actor)
            }
        }
        "IssueCommentEvent" => {
            if let Some(issue_number) = event.payload
                .get("issue")
                .and_then(|issue| issue.get("number"))
                .and_then(|n| n.as_u64())
            {
                format!("@{} commented on issue #{}", actor, issue_number)
            } else {
                format!("@{} commented on issue", actor)
            }
        }
        "PullRequestReviewEvent" => {
            if let Some(pr_number) = event.payload
                .get("pull_request")
                .and_then(|pr| pr.get("number"))
                .and_then(|n| n.as_u64())
            {
                format!("@{} reviewed PR #{}", actor, pr_number)
            } else {
                format!("@{} reviewed pull request", actor)
            }
        }
        "PullRequestReviewCommentEvent" => {
            if let Some(pr_number) = event.payload
                .get("pull_request")
                .and_then(|pr| pr.get("number"))
                .and_then(|n| n.as_u64())
            {
                format!("@{} commented on PR #{}", actor, pr_number)
            } else {
                format!("@{} commented on pull request", actor)
            }
        }
        "CreateEvent" => {
            if let Some(ref_type) = event.payload.get("ref_type").and_then(|r| r.as_str()) {
                format!("@{} created {}", actor, ref_type)
            } else {
                format!("@{} created resource", actor)
            }
        }
        "DeleteEvent" => {
            if let Some(ref_type) = event.payload.get("ref_type").and_then(|r| r.as_str()) {
                format!("@{} deleted {}", actor, ref_type)
            } else {
                format!("@{} deleted resource", actor)
            }
        }
        "ForkEvent" => format!("@{} forked repository", actor),
        "WatchEvent" => format!("@{} starred repository", actor),
        "ReleaseEvent" => {
            if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                format!("@{} {} release", actor, action)
            } else {
                format!("@{} release activity", actor)
            }
        }
        _ => format!("@{} {} event", actor, event.event_type),
    }
}
