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

    /// Generate report for the past week
    #[arg(long, conflicts_with = "since")]
    pub week: bool,

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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    
    #[test]
    fn test_cli_parsing_basic() {
        let args = vec!["gh-report"];
        let cli = Cli::parse_from(args);
        
        assert!(cli.command.is_none());
        assert!(!cli.dry_run);
        assert!(!cli.estimate_cost);
        assert!(!cli.no_cache);
        assert!(!cli.clear_cache);
        assert_eq!(cli.verbose, 0);
    }
    
    #[test]
    fn test_cli_parsing_init() {
        let args = vec!["gh-report", "init", "--lookback", "14"];
        let cli = Cli::parse_from(args);
        
        match cli.command {
            Some(Commands::Init { lookback, output }) => {
                assert_eq!(lookback, 14);
                assert!(output.is_none());
            }
            _ => panic!("Expected Init command"),
        }
    }
    
    #[test]
    fn test_cli_parsing_flags() {
        let args = vec!["gh-report", "--dry-run", "--estimate-cost", "-vv"];
        let cli = Cli::parse_from(args);
        
        assert!(cli.dry_run);
        assert!(cli.estimate_cost);
        assert_eq!(cli.verbose, 2);
    }
    
    #[test]
    fn test_cli_parsing_week_flag() {
        let args = vec!["gh-report", "--week"];
        let cli = Cli::parse_from(args);
        
        assert!(cli.week);
        assert!(cli.since.is_none());
    }
    
    #[test]
    fn test_cli_parsing_config_path() {
        let args = vec!["gh-report", "--config", "/path/to/config.toml"];
        let cli = Cli::parse_from(args);
        
        assert_eq!(cli.config, Some(PathBuf::from("/path/to/config.toml")));
    }
    
    #[test]
    fn test_cli_parsing_rebuild_state() {
        let args = vec!["gh-report", "rebuild-state"];
        let cli = Cli::parse_from(args);
        
        match cli.command {
            Some(Commands::RebuildState) => {}
            _ => panic!("Expected RebuildState command"),
        }
    }
}