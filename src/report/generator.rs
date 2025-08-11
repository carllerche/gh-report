use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use tracing::{info, warn};
use std::collections::BTreeMap;

use crate::config::Config;
use crate::github::{GitHubClient, Issue};
use crate::state::State;
use super::{Report, ReportTemplate, group_activities_by_repo};

pub struct ReportGenerator<'a> {
    client: GitHubClient,
    config: &'a Config,
    state: &'a State,
}

impl<'a> ReportGenerator<'a> {
    pub fn new(client: GitHubClient, config: &'a Config, state: &'a State) -> Self {
        ReportGenerator { client, config, state }
    }

    pub fn generate(&self, lookback_days: u32) -> Result<Report> {
        let now = Timestamp::now();
        let since = now - (lookback_days as i64 * 24).hours();
        
        info!("Generating report for the last {} days", lookback_days);
        info!("Fetching activity since {}", since.strftime("%Y-%m-%d %H:%M"));

        let mut all_issues = Vec::new();
        let mut errors = Vec::new();

        for (repo_name, _repo_state) in &self.state.tracked_repos {
            info!("Fetching issues for {}", repo_name);
            
            match self.client.fetch_issues(repo_name, Some(since)) {
                Ok(mut issues) => {
                    issues.retain(|issue| issue.updated_at >= since);
                    
                    info!("  Found {} active issues/PRs", issues.len());
                    all_issues.extend(issues);
                }
                Err(e) => {
                    warn!("Failed to fetch issues for {}: {}", repo_name, e);
                    errors.push(format!("⚠️ Could not fetch data for {}: {}", repo_name, e));
                }
            }
        }

        // TODO: Add include_mentions configuration option
        let include_mentions: Vec<String> = vec![];
        if !include_mentions.is_empty() {
            info!("Fetching mentions for users: {:?}", include_mentions);
            
            for username in &include_mentions {
                match self.fetch_user_mentions(username, since) {
                    Ok(mut mentions) => {
                        info!("  Found {} mentions for {}", mentions.len(), username);
                        all_issues.append(&mut mentions);
                    }
                    Err(e) => {
                        warn!("Failed to fetch mentions for {}: {}", username, e);
                        errors.push(format!("⚠️ Could not fetch mentions for {}: {}", username, e));
                    }
                }
            }
        }

        let activities = group_activities_by_repo(all_issues);
        
        let template = ReportTemplate::new(&self.config);
        let content = template.render(
            &activities,
            since,
            now,
            &errors,
        )?;

        let title = self.generate_title(since, now, &activities);

        Ok(Report {
            title,
            content,
            timestamp: now,
            estimated_cost: 0.0,
        })
    }

    fn fetch_user_mentions(&self, _username: &str, since: Timestamp) -> Result<Vec<Issue>> {
        self.client.fetch_mentions(since)
            .context("Failed to fetch user mentions")
    }

    fn generate_title(&self, since: Timestamp, now: Timestamp, activities: &BTreeMap<String, crate::github::RepoActivity>) -> String {
        let date_range = if since.strftime("%Y-%m-%d").to_string() == now.strftime("%Y-%m-%d").to_string() {
            format!("Daily Report - {}", now.strftime("%Y-%m-%d"))
        } else {
            format!("Report - {} to {}", 
                since.strftime("%Y-%m-%d"),
                now.strftime("%Y-%m-%d"))
        };

        let total_items: usize = activities.values()
            .map(|a| a.new_issues.len() + a.updated_issues.len() + a.new_prs.len() + a.updated_prs.len())
            .sum();

        if total_items > 0 {
            format!("{} ({} items)", date_range, total_items)
        } else {
            date_range
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::MockGitHub;

    #[test]
    fn test_report_generator_creation() {
        let mock = MockGitHub::new();
        let client = GitHubClient::Mock(mock);
        let config = Config::default();
        let state = State::default();
        
        let generator = ReportGenerator::new(client, &config, &state);
        assert!(generator.generate(1).is_ok());
    }
}