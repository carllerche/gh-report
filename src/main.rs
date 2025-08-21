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
