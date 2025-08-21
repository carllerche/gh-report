use anyhow::{Context, Result};
use clap::Parser;
use gh_report::{
    cli::{Cli, Commands},
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
        Some(Commands::Report {
            ref since,
            ref output,
            dry_run,
            estimate_cost,
            no_cache,
            clear_cache,
        }) => {
            info!("Generating activity report");
            report_command(
                since,
                output,
                dry_run,
                estimate_cost,
                no_cache,
                clear_cache,
                &cli,
            )?;
        }
        Some(Commands::Init { ref since, output }) => {
            info!("Initializing configuration based on GitHub activity");
            init_command(since, output)?;
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
        Some(Commands::ListRepos {
            ref since,
            ref output,
        }) => {
            info!("Listing repositories with recent activity");
            list_repos_command(since, output, &cli)?;
        }
        Some(Commands::Activity {
            ref since,
            ref include_types,
            ref exclude_types,
            ref output,
        }) => {
            info!("Showing GitHub activity feed");
            activity_command(
                since,
                include_types.as_ref(),
                exclude_types.as_ref(),
                output,
                &cli,
            )?;
        }
        None => {
            // Show help when no command is provided
            println!("Use --help to see available commands");
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

fn report_command(
    since: &str,
    output: &Option<PathBuf>,
    dry_run: bool,
    estimate_cost: bool,
    _no_cache: bool,
    clear_cache: bool,
    cli: &Cli,
) -> Result<()> {
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

    // Override report directory if custom output is specified
    if let Some(output_path) = output {
        if let Some(parent) = output_path.parent() {
            info!("Using custom output directory: {:?}", parent);
            config.settings.report_dir = parent.to_path_buf();

            // Create the output directory if it doesn't exist
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {:?}", parent))?;
        }
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
    if clear_cache {
        info!("Clearing cache");
        clear_cache_dir(&config)?;
    }

    // Create GitHub client for dynamic updates
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Using activity-based discovery - no need for explicit repository tracking
    println!("🔍 Discovering repositories from your GitHub activity...");

    // Dry run is now handled in the report generator

    if estimate_cost {
        info!("Estimating Claude API costs");
        estimate_costs(&config, &state)?;
        return Ok(());
    }

    // Parse the time duration using our new utility
    use gh_report::time::TimeDuration;
    let duration: TimeDuration = since
        .parse()
        .with_context(|| format!("Invalid time format: {}", since))?;
    let lookback_days = duration.as_days();

    info!("Using custom since period: {} ({})", since, duration);

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
    let report = if dry_run {
        generator
            .generate_from_activity_with_progress(lookback_days, true)
            .context("Failed to generate activity-based report (dry run)")?
    } else {
        generator
            .generate_from_activity(lookback_days)
            .context("Failed to generate activity-based report")?
    };

    // Save the report
    let report_path = if let Some(output_path) = output {
        // Custom output path specified
        report
            .save_to_path(output_path)
            .context("Failed to save report to custom path")?
    } else {
        // Use default naming and location
        report.save(&config).context("Failed to save report")?
    };

    println!("✓ Report saved to: {:?}", report_path);

    // Update state
    state.update_last_run();
    state.save(&state_file).context("Failed to save state")?;

    Ok(())
}

fn init_command(since: &str, output: Option<PathBuf>) -> Result<()> {
    let config_path = output
        .unwrap_or_else(|| Config::default_config_path().expect("Could not determine config path"));

    if config_path.exists() {
        warn!("Configuration already exists at {:?}", config_path);
        println!("Configuration file already exists at: {:?}", config_path);
        println!("Please remove it first if you want to regenerate.");
        return Ok(());
    }

    // Parse the time duration using our new utility
    use gh_report::time::TimeDuration;
    let duration: TimeDuration = since
        .parse()
        .with_context(|| format!("Invalid time format: {}", since))?;
    let _lookback_days = duration.as_days();

    println!(
        "Analyzing GitHub activity for the past {} ({})...",
        duration, since
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
    let _github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    println!("Creating configuration for activity-based GitHub reporting...");

    // Activity-based reporting doesn't need repository discovery during init
    // The activity feed will automatically find relevant repositories
    let config = Config::default();
    let state = State::default();

    println!("✓ Using activity-based repository discovery");
    println!("  Repositories will be automatically discovered from your GitHub activity");
    println!("  No manual configuration needed!");

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

fn clear_cache_dir(config: &Config) -> Result<()> {
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

fn estimate_costs(config: &Config, _state: &State) -> Result<()> {
    // TODO: Implement actual cost estimation based on data volume
    println!("Estimating costs based on current configuration...");
    println!("\nUsing activity-based repository discovery");
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

fn list_repos_command(since: &str, output: &Option<PathBuf>, _cli: &Cli) -> Result<()> {
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

    // Parse the time duration using our new utility
    use gh_report::time::TimeDuration;
    let duration: TimeDuration = since
        .parse()
        .with_context(|| format!("Invalid time format: {}", since))?;
    let lookback_days = duration.as_days();

    // Build output as a string that we can either print or write to file
    let mut output_lines = Vec::new();

    output_lines.push(format!(
        "Discovering repositories you have write access to with recent activity (last {})...",
        duration
    ));

    // Create GitHub client
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Use activity-based discovery (same as the main report)
    let all_events = github_client
        .fetch_activity(lookback_days)
        .context("Failed to fetch activity")?;

    // Apply default activity filtering
    let events = filter_events(&all_events, None, None);

    if events.is_empty() {
        output_lines.push(format!(
            "\nNo repositories found with recent activity in the last {}.",
            duration
        ));
        let final_output = output_lines.join("\n");

        if let Some(output_path) = output {
            std::fs::write(output_path, final_output)
                .with_context(|| format!("Failed to write output to {:?}", output_path))?;
            println!("Output saved to: {:?}", output_path);
        } else {
            println!("{}", final_output);
        }
        return Ok(());
    }

    // Extract unique repositories from events
    let mut repos: std::collections::HashSet<String> = std::collections::HashSet::new();
    for event in events {
        repos.insert(event.repo.name.clone());
    }

    // Group repositories by organization
    use std::collections::BTreeMap;
    let mut grouped_repos: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for repo_name in &repos {
        let (org, repo) = if let Some(slash_pos) = repo_name.find('/') {
            let org = &repo_name[..slash_pos];
            let repo = &repo_name[slash_pos + 1..];
            (org.to_string(), repo.to_string())
        } else {
            // Handle edge case of no organization
            ("(no org)".to_string(), repo_name.clone())
        };

        grouped_repos.entry(org).or_insert_with(Vec::new).push(repo);
    }

    // Sort repositories within each org alphabetically
    for repos in grouped_repos.values_mut() {
        repos.sort();
    }

    output_lines.push(format!(
        "\nFound {} repositories across {} organizations:",
        repos.len(),
        grouped_repos.len()
    ));
    output_lines.push(format!("\n{}", "=".repeat(60)));

    for (org, repos) in &grouped_repos {
        output_lines.push(format!("\n**{}** ({} repositories)", org, repos.len()));

        for repo in repos {
            output_lines.push(format!("   {}", repo));
        }
    }

    output_lines.push(format!("\n{}", "=".repeat(60)));
    output_lines
        .push("\nThese repositories have recent activity and will be automatically".to_string());
    output_lines.push("included in reports based on your GitHub activity feed.".to_string());
    output_lines.push("\nSelection criteria:".to_string());
    output_lines.push(format!("   - Recent activity in the last {}", duration));
    output_lines.push("   - Activity types: issues, PRs, comments, reviews".to_string());

    let final_output = output_lines.join("\n");

    if let Some(output_path) = output {
        std::fs::write(output_path, final_output)
            .with_context(|| format!("Failed to write output to {:?}", output_path))?;
        println!("Output saved to: {:?}", output_path);
    } else {
        println!("{}", final_output);
    }

    Ok(())
}

fn filter_events<'a>(
    events: &'a [gh_report::github::ActivityEvent],
    include_types: Option<&Vec<String>>,
    exclude_types: Option<&Vec<String>>,
) -> Vec<&'a gh_report::github::ActivityEvent> {
    let default_included_types = vec![
        "IssueCommentEvent".to_string(),
        "PullRequestEvent".to_string(),
        "IssuesEvent".to_string(),
        "PullRequestReviewCommentEvent".to_string(),
        "PullRequestReviewEvent".to_string(),
    ];

    events
        .iter()
        .filter(|event| {
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
        })
        .collect()
}

fn activity_command(
    since: &str,
    include_types: Option<&Vec<String>>,
    exclude_types: Option<&Vec<String>>,
    output: &Option<PathBuf>,
    _cli: &Cli,
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

    // Parse the time duration using our new utility
    use gh_report::time::TimeDuration;
    let duration: TimeDuration = since
        .parse()
        .with_context(|| format!("Invalid time format: {}", since))?;
    let days = duration.as_days();

    // Build output as a string that we can either print or write to file
    let mut output_lines = Vec::new();

    output_lines.push(format!(
        "Fetching activity on repositories you're subscribed to for the last {} ({})...",
        duration, since
    ));

    // Create GitHub client
    let github_client = GitHubClient::new().context("Failed to create GitHub client")?;

    // Fetch activity events
    let all_events = github_client
        .fetch_activity(days)
        .context("Failed to fetch activity")?;

    // Apply event type filtering
    let events = filter_events(&all_events, include_types, exclude_types);

    if events.is_empty() {
        output_lines.push(format!(
            "\nNo matching activity found in the last {}.",
            duration
        ));
        if all_events.len() > 0 {
            output_lines.push(format!("({} events were filtered out)", all_events.len()));
        }

        let final_output = output_lines.join("\n");

        if let Some(output_path) = output {
            std::fs::write(output_path, final_output)
                .with_context(|| format!("Failed to write output to {:?}", output_path))?;
            println!("Output saved to: {:?}", output_path);
        } else {
            println!("{}", final_output);
        }
        return Ok(());
    }

    // Group events by date → repo → issue/PR
    use std::collections::BTreeMap;

    let mut events_by_date: BTreeMap<
        String,
        BTreeMap<String, BTreeMap<Option<IssueKey>, Vec<&gh_report::github::ActivityEvent>>>,
    > = BTreeMap::new();

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

    output_lines.push(format!("\nActivity Summary ({} events):", events.len()));
    output_lines.push(format!("{}", "=".repeat(60)));

    // Display events grouped by date → repo → issue/PR
    for (date, repos_events) in events_by_date.iter().rev() {
        let total_events: usize = repos_events
            .values()
            .map(|repo_issues| {
                repo_issues
                    .values()
                    .map(|events| events.len())
                    .sum::<usize>()
            })
            .sum();
        output_lines.push(format!("\n**{}** ({} events)", date, total_events));

        for (repo_name, issues_events) in repos_events {
            let _repo_event_count: usize = issues_events.values().map(|events| events.len()).sum();
            output_lines.push(format!("  {}", repo_name));

            for (issue_key, issue_events) in issues_events {
                match issue_key {
                    Some(key) => {
                        let item_type = if key.is_pr { "PR" } else { "Issue" };
                        
                        // Extract title from the first event that has one
                        let title = issue_events
                            .iter()
                            .find_map(|event| extract_title_from_event(event))
                            .unwrap_or_else(|| "[No title]".to_string());
                        let truncated_title = truncate_title(&title, 60);
                        
                        // Show issue/PR with title
                        output_lines.push(format!(
                            "    {} #{} - {}",
                            item_type,
                            key.issue_number,
                            truncated_title
                        ));
                        
                        // Group events by action and show them indented
                        let action_groups = group_events_by_action(issue_events);
                        for (action, actors) in action_groups {
                            output_lines.push(format!(
                                "      - {} ({})",
                                action,
                                actors.join(", ")
                            ));
                        }
                    }
                    None => {
                        // Events without specific issue/PR (e.g., general repo activity)
                        for event in issue_events {
                            let event_desc = format_activity_event(event);
                            output_lines.push(format!("    {}", event_desc));
                        }
                    }
                }
            }
        }
    }

    output_lines.push(format!("\n{}", "=".repeat(60)));
    output_lines.push("\nEvent types found:".to_string());

    // Count event types
    let mut event_type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for event in &events {
        *event_type_counts
            .entry(event.event_type.clone())
            .or_insert(0) += 1;
    }

    let mut sorted_types: Vec<_> = event_type_counts.iter().collect();
    sorted_types.sort_by(|a, b| b.1.cmp(a.1));

    for (event_type, count) in sorted_types {
        output_lines.push(format!("   - {}: {}", event_type, count));
    }

    let final_output = output_lines.join("\n");

    if let Some(output_path) = output {
        std::fs::write(output_path, final_output)
            .with_context(|| format!("Failed to write output to {:?}", output_path))?;
        println!("Output saved to: {:?}", output_path);
    } else {
        println!("{}", final_output);
    }

    Ok(())
}

fn extract_issue_key(event: &gh_report::github::ActivityEvent) -> Option<IssueKey> {
    match event.event_type.as_str() {
        "PullRequestEvent" => {
            if let Some(pr_number) = event
                .payload
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
            if let Some(issue_number) = event
                .payload
                .get("issue")
                .and_then(|issue| issue.get("number"))
                .and_then(|n| n.as_u64())
            {
                // Check if this is actually a PR (issues API includes PRs)
                let is_pr = event
                    .payload
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
            if let Some(pr_number) = event
                .payload
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
                if let Some(pr_number) = event
                    .payload
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
                if let Some(issue_number) = event
                    .payload
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
            if let Some(issue_number) = event
                .payload
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
            if let Some(pr_number) = event
                .payload
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
            if let Some(pr_number) = event
                .payload
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

/// Extract title from an event payload for issues or PRs
fn extract_title_from_event(event: &gh_report::github::ActivityEvent) -> Option<String> {
    match event.event_type.as_str() {
        "PullRequestEvent" => {
            event
                .payload
                .get("pull_request")
                .and_then(|pr| pr.get("title"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        }
        "IssuesEvent" | "IssueCommentEvent" => {
            event
                .payload
                .get("issue")
                .and_then(|issue| issue.get("title"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        }
        "PullRequestReviewCommentEvent" | "PullRequestReviewEvent" => {
            event
                .payload
                .get("pull_request")
                .and_then(|pr| pr.get("title"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        }
        _ => None,
    }
}

/// Truncate a title to a reasonable length
fn truncate_title(title: &str, max_length: usize) -> String {
    if title.len() <= max_length {
        title.to_string()
    } else {
        // Account for the "..." suffix
        let content_length = max_length.saturating_sub(3);
        let truncated = &title[..content_length];
        format!("{}...", truncated)
    }
}

/// Group events by action and collect actors for each action
fn group_events_by_action(events: &[&gh_report::github::ActivityEvent]) -> Vec<(String, Vec<String>)> {
    use std::collections::HashMap;
    let mut action_actors: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    
    for event in events {
        let action_text = match event.event_type.as_str() {
            "PullRequestEvent" => {
                if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                    match action {
                        "opened" => "opened".to_string(),
                        "closed" => "closed".to_string(),
                        "reopened" => "reopened".to_string(),
                        "ready_for_review" => "ready for review".to_string(),
                        "converted_to_draft" => "converted to draft".to_string(),
                        _ => action.to_string(),
                    }
                } else {
                    "updated".to_string()
                }
            }
            "IssuesEvent" => {
                if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                    match action {
                        "opened" => "opened".to_string(),
                        "closed" => "closed".to_string(),
                        "reopened" => "reopened".to_string(),
                        _ => action.to_string(),
                    }
                } else {
                    "updated".to_string()
                }
            }
            "IssueCommentEvent" => "commented".to_string(),
            "PullRequestReviewEvent" => {
                if let Some(action) = event.payload.get("action").and_then(|a| a.as_str()) {
                    match action {
                        "submitted" => "reviewed".to_string(),
                        _ => "review activity".to_string(),
                    }
                } else {
                    "reviewed".to_string()
                }
            }
            "PullRequestReviewCommentEvent" => "review commented".to_string(),
            _ => event.event_type.clone(),
        };
        
        let actor = format!("@{}", event.actor.login);
        action_actors.entry(action_text).or_insert_with(std::collections::HashSet::new).insert(actor);
    }
    
    let mut result: Vec<(String, Vec<String>)> = action_actors
        .into_iter()
        .map(|(action, actors)| {
            let mut actor_list: Vec<String> = actors.into_iter().collect();
            actor_list.sort();
            (action, actor_list)
        })
        .collect();
    
    // Sort actions by a reasonable order
    result.sort_by(|a, b| {
        let order_a = action_priority(&a.0);
        let order_b = action_priority(&b.0);
        order_a.cmp(&order_b).then_with(|| a.0.cmp(&b.0))
    });
    
    result
}

/// Get priority order for actions (lower number = higher priority)
fn action_priority(action: &str) -> u8 {
    match action {
        "opened" => 1,
        "closed" => 2,
        "reopened" => 3,
        "reviewed" => 4,
        "commented" => 5,
        "review commented" => 6,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_title() {
        // Test short title
        let short = "Short title";
        assert_eq!(truncate_title(short, 50), "Short title");

        // Test long title  
        let long = "This is a very long title that should be truncated because it exceeds the maximum length";
        let truncated = truncate_title(long, 20);
        // 20 total chars: "This is a very lo" (17 chars) + "..." (3 chars) = 20 total
        assert_eq!(truncated, "This is a very lo...");
        assert_eq!(truncated.len(), 20);

        // Test edge case - exactly at limit
        let exact = "Exactly twenty chars";
        assert_eq!(truncate_title(exact, 20), "Exactly twenty chars");
    }

    #[test]
    fn test_action_priority() {
        assert!(action_priority("opened") < action_priority("closed"));
        assert!(action_priority("closed") < action_priority("commented"));
        assert!(action_priority("reviewed") < action_priority("unknown"));
    }
}
