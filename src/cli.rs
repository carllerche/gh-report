use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "gh-report",
    about = "Generate intelligent daily GitHub activity reports",
    version,
    author
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to configuration file
    #[arg(short, long, env = "GH_REPORT_CONFIG")]
    pub config: Option<PathBuf>,

    /// Override the automatic date detection
    #[arg(long)]
    pub since: Option<String>,

    /// Override the output file location
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Preview what would be fetched without generating report
    #[arg(long)]
    pub dry_run: bool,

    /// Show estimated Claude API cost before proceeding
    #[arg(long)]
    pub estimate_cost: bool,

    /// Bypass cache and fetch fresh data from all sources
    #[arg(long)]
    pub no_cache: bool,

    /// Clear all cached data before running
    #[arg(long)]
    pub clear_cache: bool,

    /// Verbosity level (can be repeated)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze GitHub activity and generate initial configuration
    Init {
        /// Number of days to look back
        #[arg(long, default_value = "30")]
        lookback: u32,

        /// Where to write the configuration file
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Rebuild state file from existing reports
    RebuildState,
}