use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Progress reporter for the application
pub struct ProgressReporter {
    multi: Arc<MultiProgress>,
    main_bar: Option<ProgressBar>,
    repo_bars: Vec<ProgressBar>,
    is_interactive: bool,
}

impl ProgressReporter {
    /// Create a new progress reporter
    pub fn new() -> Self {
        let is_interactive = atty::is(atty::Stream::Stdout);

        let multi = if is_interactive {
            MultiProgress::new()
        } else {
            // If not interactive, hide progress bars
            let mp = MultiProgress::new();
            mp.set_draw_target(ProgressDrawTarget::hidden());
            mp
        };

        ProgressReporter {
            multi: Arc::new(multi),
            main_bar: None,
            repo_bars: Vec::new(),
            is_interactive,
        }
    }

    /// Check if we're in an interactive terminal
    pub fn is_interactive(&self) -> bool {
        self.is_interactive
    }

    /// Start the main progress for report generation
    pub fn start_report_generation(&mut self, total_repos: usize) -> Option<ProgressBar> {
        if !self.is_interactive {
            info!(
                "Starting report generation for {} repositories",
                total_repos
            );
            return None;
        }

        let pb = self.multi.add(ProgressBar::new(total_repos as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} repos ({eta})")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        pb.set_message("Fetching GitHub data...");
        pb.enable_steady_tick(Duration::from_millis(100));

        self.main_bar = Some(pb.clone());
        Some(pb)
    }

    /// Start progress for a specific repository
    pub fn start_repo_fetch(&mut self, repo_name: &str) -> Option<ProgressBar> {
        if !self.is_interactive {
            info!("Fetching data for {}", repo_name);
            return None;
        }

        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(format!("Fetching {}", repo_name));
        pb.enable_steady_tick(Duration::from_millis(80));

        self.repo_bars.push(pb.clone());
        Some(pb)
    }

    /// Complete a repository fetch
    pub fn complete_repo_fetch(
        &self,
        pb: Option<&ProgressBar>,
        repo_name: &str,
        issue_count: usize,
    ) {
        if let Some(pb) = pb {
            pb.finish_with_message(format!("✓ {} ({} items)", repo_name, issue_count));
        } else if !self.is_interactive {
            info!("Completed {} ({} items)", repo_name, issue_count);
        }

        // Update main progress bar
        if let Some(ref main) = self.main_bar {
            main.inc(1);
        }
    }

    /// Report an error for a repository
    pub fn report_repo_error(&self, pb: Option<&ProgressBar>, repo_name: &str, error: &str) {
        if let Some(pb) = pb {
            pb.finish_with_message(format!("✗ {} - {}", repo_name, error));
        } else if !self.is_interactive {
            info!("Error fetching {}: {}", repo_name, error);
        }

        // Still increment main progress
        if let Some(ref main) = self.main_bar {
            main.inc(1);
        }
    }

    /// Start AI summarization progress
    pub fn start_ai_summary(&mut self) -> Option<ProgressBar> {
        if !self.is_interactive {
            info!("Generating AI summary...");
            return None;
        }

        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message("🤖 Generating AI summary...");
        pb.enable_steady_tick(Duration::from_millis(100));

        Some(pb)
    }

    /// Complete AI summarization
    pub fn complete_ai_summary(&self, pb: Option<&ProgressBar>, cost: f32) {
        if let Some(pb) = pb {
            pb.finish_with_message(format!("✓ AI summary generated (cost: ${:.4})", cost));
        } else if !self.is_interactive {
            info!("AI summary generated (cost: ${:.4})", cost);
        }
    }

    /// Complete the report generation
    pub fn complete_report_generation(&self, report_path: &str) {
        if let Some(ref main) = self.main_bar {
            main.finish_with_message(format!("✓ Report saved to {}", report_path));
        } else if !self.is_interactive {
            info!("Report saved to {}", report_path);
        }

        // Clear all progress bars
        if self.is_interactive {
            self.multi.clear().ok();
        }
    }

    /// Show a spinner for a long-running operation
    pub fn spinner(&self, message: &str) -> Option<ProgressBar> {
        if !self.is_interactive {
            info!("{}", message);
            return None;
        }

        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(Duration::from_millis(100));

        Some(pb)
    }
}

impl Default for ProgressReporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrap a closure with interrupt handling
pub fn with_interrupt_handler<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = interrupted.clone();

    // Set up Ctrl-C handler
    let _guard = ctrlc::set_handler(move || {
        interrupted_clone.store(true, Ordering::SeqCst);
        eprintln!("\n\n⚠️  Interrupt received. Cleaning up...");
    });

    // Run the function
    let result = f();

    // Check if we were interrupted
    if interrupted.load(Ordering::SeqCst) {
        eprintln!("\n✓ Cleanup complete. Exiting.");
        std::process::exit(130); // Standard exit code for SIGINT
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_reporter_creation() {
        let reporter = ProgressReporter::new();
        // In tests, should not be interactive
        assert!(!reporter.is_interactive());
    }

    #[test]
    fn test_progress_reporter_non_interactive() {
        let mut reporter = ProgressReporter::new();

        // These should all return None in non-interactive mode
        assert!(reporter.start_report_generation(5).is_none());
        assert!(reporter.start_repo_fetch("test/repo").is_none());
        assert!(reporter.start_ai_summary().is_none());
    }
}
