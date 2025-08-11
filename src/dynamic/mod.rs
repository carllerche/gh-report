use anyhow::Result;
use jiff::{Timestamp, ToSpan};
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::config::Config;
use crate::github::GitHubClient;
use crate::state::{State, RepoState};

mod discovery;
mod scoring;

pub use discovery::{RepositoryDiscovery, DiscoveredRepo};
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
        
        // Discover repositories with recent activity
        let discovery = RepositoryDiscovery::new(self.client);
        let lookback_days = self.config.dynamic_repos.auto_add_threshold_days;
        let discovered = discovery.discover_active_repos(lookback_days)?;
        
        info!("Discovered {} repositories with recent activity", discovered.len());
        
        // Calculate scores for discovered repos
        let mut scored_repos = Vec::new();
        for repo in discovered {
            let score = calculate_activity_score(&repo.metrics, &self.config.dynamic_repos.activity_weights);
            scored_repos.push((repo, score));
        }
        
        // Sort by score (highest first)
        scored_repos.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Determine which repos to add
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut updated = Vec::new();
        
        let min_score = self.config.dynamic_repos.min_activity_score;
        let current_repos: HashSet<String> = self.state.tracked_repos.keys().cloned().collect();
        
        // Check for new repos to add
        for (repo, score) in &scored_repos {
            if score >= &min_score && !current_repos.contains(&repo.full_name) {
                info!("Adding repository {} (score: {})", repo.full_name, score);
                self.state.tracked_repos.insert(
                    repo.full_name.clone(),
                    RepoState {
                        last_seen: repo.last_activity,
                        activity_score: *score,
                        auto_tracked: true,
                    },
                );
                added.push(repo.full_name.clone());
            } else if current_repos.contains(&repo.full_name) {
                // Update existing repo's activity
                if let Some(state) = self.state.tracked_repos.get_mut(&repo.full_name) {
                    state.last_seen = repo.last_activity;
                    state.activity_score = *score;
                    updated.push(repo.full_name.clone());
                }
            }
        }
        
        // Check for repos to remove (inactive)
        let now = Timestamp::now();
        let remove_threshold = (self.config.dynamic_repos.auto_remove_threshold_days as i64 * 24).hours();
        
        let repos_to_check: Vec<String> = self.state.tracked_repos.keys().cloned().collect();
        for repo_name in repos_to_check {
            if let Some(repo_state) = self.state.tracked_repos.get(&repo_name) {
                // Only auto-remove if it was auto-tracked
                if repo_state.auto_tracked {
                    let inactive_duration = now - repo_state.last_seen;
                    
                    if inactive_duration.get_hours() > (remove_threshold.get_hours()) || repo_state.activity_score < min_score {
                        info!("Removing inactive repository {} (score: {}, last seen: {})", 
                            repo_name, 
                            repo_state.activity_score,
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
            total_discovered: scored_repos.len(),
            total_tracked: self.state.tracked_repos.len(),
        })
    }
    
    /// Initialize repository list based on current activity
    pub fn initialize_repositories(&mut self, lookback_days: u32) -> Result<InitResult> {
        info!("Initializing repository list (lookback: {} days)", lookback_days);
        
        // Clear existing repos if any
        self.state.tracked_repos.clear();
        
        // Discover repositories
        let discovery = RepositoryDiscovery::new(self.client);
        let discovered = discovery.discover_active_repos(lookback_days)?;
        
        info!("Found {} repositories with activity", discovered.len());
        
        // Score and filter repositories
        let mut scored_repos = Vec::new();
        for repo in discovered {
            let score = calculate_activity_score(&repo.metrics, &self.config.dynamic_repos.activity_weights);
            if score >= self.config.dynamic_repos.min_activity_score {
                scored_repos.push((repo, score));
            }
        }
        
        // Sort by score
        scored_repos.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Add to state
        let mut by_score: HashMap<u32, Vec<String>> = HashMap::new();
        for (repo, score) in &scored_repos {
            self.state.tracked_repos.insert(
                repo.full_name.clone(),
                RepoState {
                    last_seen: repo.last_activity,
                    activity_score: *score,
                    auto_tracked: true,
                },
            );
            
            by_score.entry(*score).or_insert_with(Vec::new).push(repo.full_name.clone());
        }
        
        Ok(InitResult {
            total_found: scored_repos.len(),
            repositories: scored_repos.into_iter().map(|(r, s)| (r.full_name, s)).collect(),
            by_score,
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
    pub repositories: Vec<(String, u32)>,
    pub by_score: HashMap<u32, Vec<String>>,
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