use anyhow::{Context, Result};
use clap::Parser;
use gh_daily_report::{cli::{Cli, Commands}, Config, State, github::GitHubClient, report::ReportGenerator};
use std::path::PathBuf;
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
    match gh_daily_report::github::check_gh_version() {
        Ok(version) => info!("Using gh version {}", version),
        Err(e) => {
            error!("GitHub CLI check failed: {}", e);
            println!("❌ {}", e);
            println!("\nPlease install GitHub CLI from: https://cli.github.com/");
            return Err(e);
        }
    }

    info!("Loading configuration");
    let config = Config::load(cli.config.as_deref())
        .context("Failed to load configuration")?;

    info!("Loading state");
    let mut state = State::load(&config.settings.state_file)
        .context("Failed to load state")?;

    // Handle cache operations
    if cli.clear_cache {
        info!("Clearing cache");
        clear_cache(&config)?;
    }

    if cli.dry_run {
        info!("Dry run mode - showing what would be done");
        dry_run(&config, &state)?;
        return Ok(());
    }

    if cli.estimate_cost {
        info!("Estimating Claude API costs");
        estimate_costs(&config, &state)?;
        return Ok(());
    }

    // Determine the lookback days
    let lookback_days = if let Some(since_str) = &cli.since {
        info!("Using custom since date: {}", since_str);
        // For now, parse simple day count like "7d" or just a number
        if since_str.ends_with('d') {
            let days_str = &since_str[..since_str.len()-1];
            days_str.parse::<u32>()
                .with_context(|| format!("Invalid since format: {}", since_str))?
        } else {
            since_str.parse::<u32>()
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
    
    // Create GitHub client
    println!("📊 Fetching GitHub activity...");
    let github_client = GitHubClient::new()
        .context("Failed to create GitHub client")?;
    
    // Generate the report
    let generator = ReportGenerator::new(github_client, &config, &state);
    let report = generator.generate(lookback_days)
        .context("Failed to generate report")?;
    
    // Save the report
    let report_path = report.save(&config)
        .context("Failed to save report")?;
    
    println!("✓ Report saved to: {:?}", report_path);
    
    // Update state
    state.update_last_run();
    state.save(&config.settings.state_file)
        .context("Failed to save state")?;

    Ok(())
}

fn init_command(lookback: u32, output: Option<PathBuf>) -> Result<()> {
    let config_path = output.unwrap_or_else(|| {
        Config::default_config_path()
            .expect("Could not determine config path")
    });

    if config_path.exists() {
        warn!("Configuration already exists at {:?}", config_path);
        println!("Configuration file already exists at: {:?}", config_path);
        println!("Please remove it first if you want to regenerate.");
        return Ok(());
    }

    println!("Analyzing GitHub activity for the past {} days...", lookback);
    
    // TODO: Implement GitHub activity analysis (Milestone 6)
    println!("⚠️  GitHub analysis not yet implemented (Milestone 6)");
    println!("Creating default configuration instead...");

    // Create a default configuration for now
    let config = Config::default();
    let config_str = toml::to_string_pretty(&config)
        .context("Failed to serialize config")?;

    std::fs::write(&config_path, config_str)
        .with_context(|| format!("Failed to write config to {:?}", config_path))?;

    println!("✓ Configuration created at: {:?}", config_path);
    println!("\nNext steps:");
    println!("1. Set your Anthropic API key:");
    println!("   export ANTHROPIC_API_KEY='your-key-here'");
    println!("2. Review and customize the configuration file");
    println!("3. Run 'gh-report' to generate your first report");

    Ok(())
}

fn rebuild_state_command(cli: &Cli) -> Result<()> {
    let config = Config::load(cli.config.as_deref())
        .context("Failed to load configuration")?;

    println!("Scanning report directory: {:?}", config.settings.report_dir);
    
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

fn dry_run(config: &Config, state: &State) -> Result<()> {
    println!("✓ Configuration loaded");
    
    if let Some(last_run) = state.last_run {
        println!("✓ Last report: {}", last_run);
    } else {
        println!("✓ First run - no previous report");
    }

    println!("\nWould check {} repositories:", state.tracked_repos.len());
    
    // Group repos by importance
    let mut by_importance = std::collections::BTreeMap::new();
    for (name, _repo_state) in &state.tracked_repos {
        // TODO: Get actual importance from config
        let importance = "unknown";
        by_importance.entry(importance).or_insert(Vec::new()).push(name);
    }

    for (importance, repos) in by_importance {
        println!("\n  {} priority ({}):", importance, repos.len());
        for repo in repos.iter().take(3) {
            println!("    - {}", repo);
        }
        if repos.len() > 3 {
            println!("    ... and {} more", repos.len() - 3);
        }
    }

    println!("\nWould fetch:");
    println!("  - Issues/PRs from last {} days", config.settings.max_lookback_days);
    println!("  - Comments on open issues/PRs");
    println!("  - Mentions across GitHub");

    println!("\nEstimated:");
    println!("  - GitHub API calls: ~{}", state.tracked_repos.len() * 3);
    println!("  - Claude API calls: 3-5");
    println!("  - Estimated cost: $0.02-0.04");
    println!("  - Estimated time: 30-45 seconds");

    Ok(())
}

fn estimate_costs(config: &Config, state: &State) -> Result<()> {
    // TODO: Implement actual cost estimation based on data volume
    println!("Estimating costs based on current configuration...");
    println!("\nRepositories to check: {}", state.tracked_repos.len());
    println!("Max issues per report: {}", config.settings.max_issues_per_report);
    println!("Max comments per report: {}", config.settings.max_comments_per_report);
    
    println!("\nEstimated Claude API usage:");
    println!("  Primary model ({}): ~5000 tokens", config.claude.primary_model);
    println!("  Secondary model ({}): ~2000 tokens", config.claude.secondary_model);
    println!("\nEstimated cost: $0.02-0.04");
    
    Ok(())
}
