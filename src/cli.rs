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

    /// Override the state file location
    #[arg(long)]
    pub state: Option<PathBuf>,

    /// Verbosity level (can be repeated)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate activity report
    Report {
        /// Time period to look back (e.g., 3d, 12h, 2w)
        #[arg(long, default_value = "7d")]
        since: String,

        /// Override the output file location  
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Preview what would be fetched without generating report
        #[arg(long)]
        dry_run: bool,

        /// Show estimated Claude API cost before proceeding
        #[arg(long)]
        estimate_cost: bool,

        /// Bypass cache and fetch fresh data from all sources
        #[arg(long)]
        no_cache: bool,

        /// Clear all cached data before running
        #[arg(long)]
        clear_cache: bool,
    },

    /// Analyze GitHub activity and generate initial configuration
    Init {
        /// Time period to look back (e.g., 30d, 4w, 720h)
        #[arg(long, default_value = "30d")]
        since: String,

        /// Where to write the configuration file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Rebuild state file from existing reports
    RebuildState,

    /// Summarize a specific GitHub issue or PR
    Summarize {
        /// Issue or PR reference (URL or shorthand like "owner/repo#123")
        target: String,

        /// Custom output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Skip AI recommendations and focus on facts only
        #[arg(long)]
        no_recommendations: bool,
    },

    /// List repositories with recent activity (preview for init)
    ListRepos {
        /// Time period to look back (e.g., 30d, 4w, 720h)
        #[arg(long, default_value = "30d")]
        since: String,

        /// Save the list to a file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show your GitHub activity feed
    Activity {
        /// Time period to look back (e.g., 7d, 12h, 2w)
        #[arg(long, default_value = "7d")]
        since: String,

        /// Include only these event types (comma-separated)
        /// Examples: IssueCommentEvent,PullRequestEvent,IssuesEvent
        #[arg(long, value_delimiter = ',')]
        include_types: Option<Vec<String>>,

        /// Exclude these event types (comma-separated)
        /// Examples: WatchEvent,ForkEvent,PushEvent
        #[arg(long, value_delimiter = ',')]
        exclude_types: Option<Vec<String>>,

        /// Save the activity to a file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
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
        assert_eq!(cli.verbose, 0);
    }

    #[test]
    fn test_cli_parsing_init() {
        let args = vec!["gh-report", "init", "--since", "14d"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Init { since, output }) => {
                assert_eq!(since, "14d");
                assert!(output.is_none());
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_cli_parsing_report_flags() {
        let args = vec![
            "gh-report",
            "report",
            "--dry-run",
            "--estimate-cost",
            "--since",
            "3d",
        ];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Report {
                since,
                dry_run,
                estimate_cost,
                ..
            }) => {
                assert_eq!(since, "3d");
                assert!(dry_run);
                assert!(estimate_cost);
            }
            _ => panic!("Expected Report command"),
        }
    }

    #[test]
    fn test_cli_parsing_report_with_output() {
        let args = vec!["gh-report", "report", "--output", "/tmp/custom-report.md"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Report { output, .. }) => {
                assert_eq!(output, Some(PathBuf::from("/tmp/custom-report.md")));
            }
            _ => panic!("Expected Report command"),
        }
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

    #[test]
    fn test_cli_parsing_summarize() {
        let args = vec!["gh-report", "summarize", "tokio-rs/tokio#123"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Summarize {
                target,
                output,
                no_recommendations,
            }) => {
                assert_eq!(target, "tokio-rs/tokio#123");
                assert!(output.is_none());
                assert!(!no_recommendations);
            }
            _ => panic!("Expected Summarize command"),
        }
    }

    #[test]
    fn test_cli_parsing_summarize_with_options() {
        let args = vec![
            "gh-report",
            "summarize",
            "https://github.com/rust-lang/rust/issues/123",
            "--output",
            "/tmp/summary.md",
            "--no-recommendations",
        ];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Summarize {
                target,
                output,
                no_recommendations,
            }) => {
                assert_eq!(target, "https://github.com/rust-lang/rust/issues/123");
                assert_eq!(output, Some(PathBuf::from("/tmp/summary.md")));
                assert!(no_recommendations);
            }
            _ => panic!("Expected Summarize command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_repos() {
        let args = vec!["gh-report", "list-repos"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::ListRepos { since, output }) => {
                assert_eq!(since, "30d"); // default value
                assert!(output.is_none());
            }
            _ => panic!("Expected ListRepos command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_repos_with_since() {
        let args = vec!["gh-report", "list-repos", "--since", "14d"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::ListRepos { since, output }) => {
                assert_eq!(since, "14d");
                assert!(output.is_none());
            }
            _ => panic!("Expected ListRepos command"),
        }
    }

    #[test]
    fn test_cli_parsing_activity() {
        let args = vec!["gh-report", "activity"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Activity {
                since,
                include_types,
                exclude_types,
                output,
            }) => {
                assert_eq!(since, "7d"); // default value
                assert!(include_types.is_none());
                assert!(exclude_types.is_none());
                assert!(output.is_none());
            }
            _ => panic!("Expected Activity command"),
        }
    }

    #[test]
    fn test_cli_parsing_activity_with_since() {
        let args = vec!["gh-report", "activity", "--since", "14d"];
        let cli = Cli::parse_from(args);

        match cli.command {
            Some(Commands::Activity {
                since,
                include_types,
                exclude_types,
                output,
            }) => {
                assert_eq!(since, "14d");
                assert!(include_types.is_none());
                assert!(exclude_types.is_none());
                assert!(output.is_none());
            }
            _ => panic!("Expected Activity command"),
        }
    }
}
