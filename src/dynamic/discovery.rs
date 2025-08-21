use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use serde::Deserialize;
use std::process::Command;
use tracing::{debug, info};

use crate::dynamic::ActivityMetrics;
use crate::github::GitHubClient;

/// Discovers repositories with recent activity
pub struct RepositoryDiscovery<'a> {
    client: &'a GitHubClient,
}

impl<'a> RepositoryDiscovery<'a> {
    pub fn new(client: &'a GitHubClient) -> Self {
        RepositoryDiscovery { client }
    }

    /// Discover repositories with recent activity that user has write access to
    pub fn discover_active_repos(&self, _lookback_days: u32) -> Result<Vec<DiscoveredRepo>> {
        // Get current user
        let username = self
            .client
            .get_current_user()
            .context("Failed to get current user")?;

        info!("Discovering repositories for user: {}", username);

        // Step 1: Find repositories where user has recent activity (last 30 days)
        let recent_activity_repos = self.find_repos_with_recent_activity(&username)?;
        info!(
            "Found {} repositories with recent activity",
            recent_activity_repos.len()
        );

        // Step 2: Filter to only repositories where user has write permissions
        let mut filtered_repos = Vec::new();
        for repo in recent_activity_repos {
            if self.user_has_write_access(&username, &repo.full_name)? {
                info!(
                    "Repository {} passed write permission check",
                    repo.full_name
                );
                filtered_repos.push(repo);
            } else {
                info!("Repository {} excluded - no write access", repo.full_name);
            }
        }

        info!(
            "Found {} repositories after filtering for write access",
            filtered_repos.len()
        );

        Ok(filtered_repos)
    }

    /// Find repositories where user has had any recent activity (last 30 days)
    fn find_repos_with_recent_activity(&self, username: &str) -> Result<Vec<DiscoveredRepo>> {
        let mut repos = Vec::new();
        let recent_date = days_ago_date(30); // Always use 30 days for recent activity

        // Search for repositories with recent activity
        // We'll use multiple search queries to find relevant repos

        // 1. Repositories where user is involved (broader search)
        let involves_query = format!("involves:{} updated:>{}", username, recent_date);
        repos.extend(self.search_repos(&involves_query)?);

        // 2. Repositories where user has recently created issues/PRs
        let author_query = format!("author:{} updated:>{}", username, recent_date);
        repos.extend(self.search_repos(&author_query)?);

        // 3. Repositories where user was mentioned
        let mention_query = format!("mentions:{} updated:>{}", username, recent_date);
        repos.extend(self.search_repos(&mention_query)?);

        // 4. Repositories where user has been assigned
        let assignee_query = format!("assignee:{} updated:>{}", username, recent_date);
        repos.extend(self.search_repos(&assignee_query)?);

        // 5. Repositories where user has reviewed PRs
        let review_query = format!("reviewed-by:{} updated:>{}", username, recent_date);
        repos.extend(self.search_repos(&review_query)?);

        // Deduplicate repositories
        let mut unique_repos = std::collections::HashMap::new();
        for repo in repos {
            unique_repos
                .entry(repo.full_name.clone())
                .and_modify(|existing: &mut DiscoveredRepo| {
                    // Merge metrics
                    existing.metrics.commits += repo.metrics.commits;
                    existing.metrics.prs += repo.metrics.prs;
                    existing.metrics.issues += repo.metrics.issues;
                    existing.metrics.comments += repo.metrics.comments;
                    // Keep most recent activity
                    if repo.last_activity > existing.last_activity {
                        existing.last_activity = repo.last_activity;
                    }
                })
                .or_insert(repo);
        }

        Ok(unique_repos.into_values().collect())
    }

    /// Check if user has write access to a repository
    fn user_has_write_access(&self, _username: &str, repo: &str) -> Result<bool> {
        // For mock implementation, return true for testing
        #[allow(unreachable_patterns)]
        match self.client {
            #[cfg(test)]
            GitHubClient::Mock(_) => return Ok(true),
            _ => {}
        }

        // Use gh api to check repository permissions
        let mut cmd = Command::new("gh");
        cmd.arg("api")
            .arg(format!("/repos/{}", repo))
            .arg("--jq")
            .arg(".permissions.push // false"); // Check if user has push (write) access

        debug!("Checking write access for {}", repo);
        let output = cmd.output().context("Failed to execute gh api")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("Failed to check permissions for {}: {}", repo, stderr);
            // If we can't check, err on the side of exclusion for permissions
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Permission check output for {}: {}", repo, stdout);

        // Parse the permission result
        let has_write_access = stdout.trim() == "true";
        debug!("User has write access to {}: {}", repo, has_write_access);

        Ok(has_write_access)
    }

    /// Search for repositories using a query
    fn search_repos(&self, query: &str) -> Result<Vec<DiscoveredRepo>> {
        debug!("Searching with query: {}", query);

        // For mock implementation, return empty for now
        // Real implementation would use gh search
        #[allow(unreachable_patterns)]
        match self.client {
            #[cfg(test)]
            GitHubClient::Mock(_) => return Ok(vec![]),
            _ => {}
        }

        // Real implementation uses gh search
        {
            // Use gh search to find repositories
            // We need to split the query into parts for gh search to handle correctly
            let mut cmd = Command::new("gh");
            cmd.arg("search").arg("issues");

            // Split the query and add each part as separate arguments
            // This prevents gh from wrapping the entire query in quotes
            for part in query.split_whitespace() {
                cmd.arg(part);
            }

            cmd.arg("--json")
                .arg("repository,updatedAt")
                .arg("--limit")
                .arg("100");

            info!("Executing gh search command with query: {}", query);
            let output = cmd.output().context("Failed to execute gh search")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                info!("gh search returned non-zero status. stderr: {}", stderr);
                // Search might return no results, which is okay
                return Ok(vec![]);
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("gh search raw output: {}", stdout);

            let results: Vec<SearchResult> =
                serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
                    info!("Failed to parse gh search JSON: {}", e);
                    vec![]
                });

            info!("gh search found {} results", results.len());

            // Convert search results to discovered repos
            let mut repos = Vec::new();
            let mut seen = std::collections::HashSet::new();

            for result in results {
                if !seen.contains(&result.repository.full_name) {
                    seen.insert(result.repository.full_name.clone());
                    debug!("Found repository: {}", result.repository.full_name);
                    repos.push(DiscoveredRepo {
                        full_name: result.repository.full_name,
                        last_activity: result.updated_at,
                        metrics: ActivityMetrics::default(),
                    });
                }
            }

            info!("Returning {} unique repositories from search", repos.len());
            Ok(repos)
        }
    }
}

/// Get date string for N days ago
fn days_ago_date(days: u32) -> String {
    let date = Timestamp::now() - (days as i64 * 24).hours();
    date.strftime("%Y-%m-%d").to_string()
}

/// A discovered repository with activity metrics
#[derive(Debug, Clone)]
pub struct DiscoveredRepo {
    pub full_name: String,
    pub last_activity: Timestamp,
    pub metrics: ActivityMetrics,
}

/// Search result from gh search
#[derive(Debug, Deserialize)]
struct SearchResult {
    repository: SearchRepo,
    #[serde(rename = "updatedAt")]
    updated_at: Timestamp,
}

#[derive(Debug, Deserialize)]
struct SearchRepo {
    #[serde(rename = "nameWithOwner")]
    full_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_discovery() {
        let client = GitHubClient::mock();
        let discovery = RepositoryDiscovery::new(&client);

        let repos = discovery.discover_active_repos(7).unwrap();
        // Mock returns empty list
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_days_ago_date() {
        let date = days_ago_date(7);
        // Should be a valid date string
        assert!(date.contains('-'));
        assert_eq!(date.len(), 10); // YYYY-MM-DD
    }
}
