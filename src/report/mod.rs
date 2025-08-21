use anyhow::{Context, Result};
use jiff::Timestamp;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::github::{Issue, RepoActivity};

mod generator;
mod template;

pub use generator::ReportGenerator;
pub use template::ReportTemplate;

/// A generated report ready to be saved
pub struct Report {
    pub title: String,
    pub content: String,
    pub timestamp: Timestamp,
    pub estimated_cost: f32,
}

impl Report {
    /// Save the report to a file
    pub fn save(&self, config: &Config) -> Result<PathBuf> {
        // Ensure report directory exists
        let report_dir = &config.settings.report_dir;
        fs::create_dir_all(report_dir)
            .with_context(|| format!("Failed to create report directory: {:?}", report_dir))?;

        // Generate filename
        let filename = self.generate_filename(config);
        let filepath = report_dir.join(&filename);

        // Write report
        fs::write(&filepath, &self.content)
            .with_context(|| format!("Failed to write report to {:?}", filepath))?;

        Ok(filepath)
    }

    /// Generate filename based on config format
    fn generate_filename(&self, config: &Config) -> String {
        let mut filename = config.settings.file_name_format.clone();

        // Replace date placeholders
        let date_str = self.timestamp.strftime("%Y-%m-%d").to_string();
        filename = filename.replace("{yyyy-mm-dd}", &date_str);

        // Extract year, month, day for individual replacements
        let year = self.timestamp.strftime("%Y").to_string();
        let month = self.timestamp.strftime("%m").to_string();
        let day = self.timestamp.strftime("%d").to_string();

        filename = filename.replace("{yyyy}", &year);
        filename = filename.replace("{mm}", &month);
        filename = filename.replace("{dd}", &day);

        // Generate short title (max 8 words)
        let short_title = self.generate_short_title();
        filename = filename.replace("{short-title}", &short_title);

        // Ensure .md extension
        if !filename.ends_with(".md") {
            filename.push_str(".md");
        }

        filename
    }

    /// Generate a short title from the report content
    fn generate_short_title(&self) -> String {
        // For now, use a simple heuristic based on the main title
        // In the future, this could use AI to generate a better summary
        let words: Vec<&str> = self.title.split_whitespace().take(8).collect();

        if words.is_empty() {
            "Daily Report".to_string()
        } else {
            words.join(" ")
        }
    }
}

/// Group activities by repository
pub fn group_activities_by_repo(issues: Vec<Issue>) -> BTreeMap<String, RepoActivity> {
    let mut activities: BTreeMap<String, RepoActivity> = BTreeMap::new();

    for issue in issues {
        // Extract repo name from URL (format: https://github.com/owner/repo/...)
        let repo_name = extract_repo_from_url(&issue.url).unwrap_or_else(|| "unknown".to_string());

        let activity = activities
            .entry(repo_name)
            .or_insert_with(RepoActivity::default);

        // Categorize by type and state
        if issue.is_pull_request {
            match issue.state {
                crate::github::IssueState::Open => {
                    if issue.created_at.as_second() > Timestamp::now().as_second() - 86400 {
                        activity.new_prs.push(issue);
                    } else {
                        activity.updated_prs.push(issue);
                    }
                }
                _ => activity.updated_prs.push(issue),
            }
        } else {
            match issue.state {
                crate::github::IssueState::Open => {
                    if issue.created_at.as_second() > Timestamp::now().as_second() - 86400 {
                        activity.new_issues.push(issue);
                    } else {
                        activity.updated_issues.push(issue);
                    }
                }
                _ => activity.updated_issues.push(issue),
            }
        }
    }

    activities
}

/// Extract repository name from GitHub URL
fn extract_repo_from_url(url: &str) -> Option<String> {
    // URL format: https://github.com/owner/repo/...
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() >= 5 && parts[2] == "github.com" {
        Some(format!("{}/{}", parts[3], parts[4]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_repo_from_url() {
        assert_eq!(
            extract_repo_from_url("https://github.com/rust-lang/rust/issues/123"),
            Some("rust-lang/rust".to_string())
        );

        assert_eq!(
            extract_repo_from_url("https://github.com/owner/repo/pull/456"),
            Some("owner/repo".to_string())
        );

        assert_eq!(extract_repo_from_url("https://example.com/foo/bar"), None);
    }

    #[test]
    fn test_generate_filename() {
        let report = Report {
            title: "Test Report Title Here".to_string(),
            content: "# Test".to_string(),
            timestamp: Timestamp::from_second(1704931200).unwrap(), // 2024-01-11
            estimated_cost: 0.0,
        };

        let config = Config::default();
        let filename = report.generate_filename(&config);

        assert!(filename.contains("2024-01-11"));
        assert!(filename.contains("Test Report Title Here"));
        assert!(filename.ends_with(".md"));
    }
}
