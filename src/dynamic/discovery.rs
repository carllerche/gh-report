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

    /// Discover repositories with recent activity
    pub fn discover_active_repos(&self, lookback_days: u32) -> Result<Vec<DiscoveredRepo>> {
        let mut repos = Vec::new();

        // Get current user
        let username = self
            .client
            .get_current_user()
            .context("Failed to get current user")?;

        info!("Discovering repositories for user: {}", username);

        // Search for repositories with recent activity
        // We'll use multiple search queries to find relevant repos

        // 1. Repositories where user is involved (broader search)
        let involves_query = format!(
            "involves:{} updated:>{}",
            username,
            days_ago_date(lookback_days)
        );
        repos.extend(self.search_repos(&involves_query)?);

        // 2. Repositories where user has recently created issues/PRs
        let author_query = format!(
            "author:{} updated:>{}",
            username,
            days_ago_date(lookback_days)
        );
        repos.extend(self.search_repos(&author_query)?);

        // 3. Repositories where user was mentioned
        let mention_query = format!(
            "mentions:{} updated:>{}",
            username,
            days_ago_date(lookback_days)
        );
        repos.extend(self.search_repos(&mention_query)?);

        // 4. Repositories where user has been assigned
        let assignee_query = format!(
            "assignee:{} updated:>{}",
            username,
            days_ago_date(lookback_days)
        );
        repos.extend(self.search_repos(&assignee_query)?);

        // 5. Repositories where user has reviewed PRs
        let review_query = format!(
            "reviewed-by:{} updated:>{}",
            username,
            days_ago_date(lookback_days)
        );
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

        let mut result: Vec<DiscoveredRepo> = unique_repos.into_values().collect();

        // Fetch additional metrics for each repository
        for repo in &mut result {
            self.enrich_repo_metrics(repo, lookback_days)?;
        }

        Ok(result)
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

    /// Enrich repository with additional metrics
    fn enrich_repo_metrics(&self, repo: &mut DiscoveredRepo, lookback_days: u32) -> Result<()> {
        // Get recent issues and PRs to calculate metrics
        let since = Timestamp::now() - (lookback_days as i64 * 24).hours();

        info!("Enriching metrics for repository: {}", repo.full_name);
        match self.client.fetch_issues(&repo.full_name, Some(since)) {
            Ok(issues) => {
                info!("Fetched {} issues/PRs for {}", issues.len(), repo.full_name);
                for issue in issues {
                    if issue.is_pull_request {
                        repo.metrics.prs += 1;
                    } else {
                        repo.metrics.issues += 1;
                    }
                    repo.metrics.comments += issue.comments.total_count;
                }
            }
            Err(e) => {
                // Log as info instead of debug so we can see the errors
                info!("Failed to fetch issues for {}: {}", repo.full_name, e);
            }
        }

        // Note: Commit count would require additional API calls
        // For now, we'll estimate based on PR activity
        repo.metrics.commits = repo.metrics.prs * 3; // Rough estimate

        info!(
            "Metrics for {}: commits={}, prs={}, issues={}, comments={}",
            repo.full_name,
            repo.metrics.commits,
            repo.metrics.prs,
            repo.metrics.issues,
            repo.metrics.comments
        );

        Ok(())
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
    use crate::github::MockGitHub;

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
