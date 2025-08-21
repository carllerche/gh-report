use anyhow::Result;
use jiff::{Timestamp, ToSpan};
use std::collections::HashSet;
use tracing::info;

use crate::config::Config;
use crate::github::GitHubClient;
use crate::state::{RepoState, State};

mod discovery;
mod scoring;

pub use discovery::{DiscoveredRepo, RepositoryDiscovery};
pub use scoring::{calculate_activity_score, ActivityMetrics};

/// Manages dynamic repository tracking
pub struct DynamicRepoManager<'a> {
    config: &'a Config,
    state: &'a mut State,
    client: &'a GitHubClient,
}

impl<'a> DynamicRepoManager<'a> {
    pub fn new(config: &'a Config, state: &'a mut State, client: &'a GitHubClient) -> Self {
        DynamicRepoManager {
            config,
            state,
            client,
        }
    }

    /// Update repository list based on activity
    pub fn update_repositories(&mut self) -> Result<RepoUpdateResult> {
        if !self.config.dynamic_repos.enabled {
            info!("Dynamic repository management is disabled");
            return Ok(RepoUpdateResult::default());
        }

        info!("Updating dynamic repository list");

        // Discover repositories with recent activity and write access
        let discovery = RepositoryDiscovery::new(self.client);
        let discovered = discovery.discover_active_repos(30)?; // Always use 30 days

        info!(
            "Discovered {} repositories with recent activity and write access",
            discovered.len()
        );

        // Determine which repos to add
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut updated = Vec::new();

        let current_repos: HashSet<String> = self.state.tracked_repos.keys().cloned().collect();

        // Check for new repos to add
        for repo in &discovered {
            if !current_repos.contains(&repo.full_name) {
                info!("Adding repository {}", repo.full_name);
                self.state.tracked_repos.insert(
                    repo.full_name.clone(),
                    RepoState {
                        last_seen: repo.last_activity,
                        activity_score: 0, // No longer used
                        auto_tracked: true,
                    },
                );
                added.push(repo.full_name.clone());
            } else {
                // Update existing repo's activity
                if let Some(state) = self.state.tracked_repos.get_mut(&repo.full_name) {
                    state.last_seen = repo.last_activity;
                    updated.push(repo.full_name.clone());
                }
            }
        }

        // Check for repos to remove (inactive)
        let now = Timestamp::now();
        let remove_threshold =
            (self.config.dynamic_repos.auto_remove_threshold_days as i64 * 24).hours();

        let repos_to_check: Vec<String> = self.state.tracked_repos.keys().cloned().collect();
        for repo_name in repos_to_check {
            if let Some(repo_state) = self.state.tracked_repos.get(&repo_name) {
                // Only auto-remove if it was auto-tracked
                if repo_state.auto_tracked {
                    let inactive_duration = now - repo_state.last_seen;

                    if inactive_duration.get_hours() > (remove_threshold.get_hours()) {
                        info!(
                            "Removing inactive repository {} (last seen: {})",
                            repo_name,
                            repo_state.last_seen.strftime("%Y-%m-%d")
                        );
                        self.state.tracked_repos.remove(&repo_name);
                        removed.push(repo_name);
                    }
                }
            }
        }

        Ok(RepoUpdateResult {
            added,
            removed,
            updated,
            total_discovered: discovered.len(),
            total_tracked: self.state.tracked_repos.len(),
        })
    }

    /// Initialize repository list based on current activity
    pub fn initialize_repositories(&mut self, _lookback_days: u32) -> Result<InitResult> {
        info!("Initializing repository list");

        // Clear existing repos if any
        self.state.tracked_repos.clear();

        // Discover repositories with recent activity and write access (always 30 days)
        let discovery = RepositoryDiscovery::new(self.client);
        let discovered = discovery.discover_active_repos(30)?;

        info!(
            "Found {} repositories with activity and write access",
            discovered.len()
        );

        // Add all discovered repositories to state (no scoring)
        let mut repositories = Vec::new();
        for repo in discovered {
            self.state.tracked_repos.insert(
                repo.full_name.clone(),
                RepoState {
                    last_seen: repo.last_activity,
                    activity_score: 0, // No longer used
                    auto_tracked: true,
                },
            );
            repositories.push((repo.full_name, 0)); // Score is always 0 now
        }

        Ok(InitResult {
            total_found: repositories.len(),
            repositories,
        })
    }
}

/// Result of updating repository list
#[derive(Debug, Default)]
pub struct RepoUpdateResult {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<String>,
    pub total_discovered: usize,
    pub total_tracked: usize,
}

/// Result of initializing repository list
#[derive(Debug)]
pub struct InitResult {
    pub total_found: usize,
    pub repositories: Vec<(String, u32)>, // Score is always 0 now, kept for compatibility
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::MockGitHub;

    #[test]
    fn test_dynamic_repo_manager_creation() {
        let config = Config::default();
        let mut state = State::default();
        let client = GitHubClient::mock();

        let manager = DynamicRepoManager::new(&config, &mut state, &client);
        assert!(manager.config.dynamic_repos.enabled);
    }

    #[test]
    fn test_repo_update_with_disabled() {
        let mut config = Config::default();
        config.dynamic_repos.enabled = false;

        let mut state = State::default();
        let client = GitHubClient::mock();

        let mut manager = DynamicRepoManager::new(&config, &mut state, &client);
        let result = manager.update_repositories().unwrap();

        assert_eq!(result.added.len(), 0);
        assert_eq!(result.removed.len(), 0);
    }
}
