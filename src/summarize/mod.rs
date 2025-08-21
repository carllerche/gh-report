use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tracing::{info, warn};

use crate::claude::prompts::{
    generate_issue_filename, review_pr_for_maintainer, summarize_issue_for_maintainer,
};
use crate::claude::{resolve_model_alias, ClaudeInterface, Message, MessagesRequest};
use crate::config::Config;
use crate::github::{parse_issue_reference, Comment, GitHubClient, Issue, IssueState};

/// Orchestrates the summarization of a specific GitHub issue or PR
pub struct IssueSummarizer<'a> {
    github_client: GitHubClient,
    claude_client: Option<ClaudeInterface>,
    config: &'a Config,
}

impl<'a> IssueSummarizer<'a> {
    /// Create a new issue summarizer
    pub fn new(github_client: GitHubClient, config: &'a Config) -> Self {
        // Try to create Claude client
        let claude_client = match ClaudeInterface::new(&config.claude) {
            Ok(client) => client,
            Err(e) => {
                warn!("Failed to initialize Claude: {}", e);
                None
            }
        };

        IssueSummarizer {
            github_client,
            claude_client,
            config,
        }
    }

    /// Summarize an issue or PR and save to file
    pub fn summarize(
        &self,
        target: &str,
        output_path: Option<&Path>,
        include_recommendations: bool,
    ) -> Result<String> {
        info!("Parsing issue reference: {}", target);

        // Parse the issue reference
        let reference = parse_issue_reference(target)
            .with_context(|| format!("Failed to parse issue reference: {}", target))?;

        info!(
            "Fetching issue #{} from {}",
            reference.number,
            reference.repo_name()
        );

        // Fetch the issue and comments
        let (issue, comments) = self
            .github_client
            .fetch_single_issue(&reference.repo_name(), reference.number)
            .with_context(|| {
                format!(
                    "Failed to fetch issue #{} from {}",
                    reference.number,
                    reference.repo_name()
                )
            })?;

        info!("Fetched issue with {} comments", comments.len());

        // Generate the summary
        let summary = if let Some(claude) = &self.claude_client {
            self.generate_ai_summary(claude, &issue, &comments, include_recommendations)?
        } else {
            warn!("Claude not available, generating basic summary");
            self.generate_basic_summary(&issue, &comments)
        };

        // Determine output file path
        let output_file = if let Some(path) = output_path {
            path.to_path_buf()
        } else {
            // Generate filename in current directory
            let filename =
                generate_issue_filename(&reference.repo_name(), reference.number, &issue.title);
            std::env::current_dir()?.join(filename)
        };

        // Write the summary to file
        std::fs::write(&output_file, &summary)
            .with_context(|| format!("Failed to write summary to {}", output_file.display()))?;

        info!("Summary saved to: {}", output_file.display());

        Ok(output_file.to_string_lossy().into_owned())
    }

    /// Generate AI-powered summary using Claude
    fn generate_ai_summary(
        &self,
        claude: &ClaudeInterface,
        issue: &Issue,
        comments: &[Comment],
        include_recommendations: bool,
    ) -> Result<String> {
        // Prepare issue data
        let issue_state = match issue.state {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::Merged => "merged",
        };

        let issue_labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
        let issue_body = issue.body.as_deref().unwrap_or("No description provided.");

        // Convert comments to (author, body) pairs
        let comment_pairs: Vec<(String, String)> = comments
            .iter()
            .map(|c| (c.author.login.clone(), c.body.clone()))
            .collect();

        // Generate the prompt based on whether this is a PR or issue
        let prompt = if issue.is_pull_request {
            // For PRs, get diff information and use code review prompt
            let diff_summary = self.get_pr_diff_summary(issue)?;
            review_pr_for_maintainer(
                &issue.title,
                issue_body,
                issue_state,
                &issue.author.login,
                &issue_labels,
                &issue.url,
                &comment_pairs,
                &diff_summary,
                include_recommendations,
            )
        } else {
            // For issues, use the regular issue prompt
            summarize_issue_for_maintainer(
                &issue.title,
                issue_body,
                issue_state,
                &issue.author.login,
                &issue_labels,
                &issue.url,
                &comment_pairs,
                include_recommendations,
            )
        };

        // Call Claude
        let model = resolve_model_alias(&self.config.claude.primary_model);
        let request =
            MessagesRequest::new(model, vec![Message::user(prompt)]).with_max_tokens(4000);

        let response = claude
            .messages(request)
            .context("Failed to get summary from Claude")?;

        // Generate the final markdown with header
        let ai_summary = response.get_text();
        Ok(self.format_final_summary(issue, &ai_summary))
    }

    /// Generate basic summary without AI
    fn generate_basic_summary(&self, issue: &Issue, comments: &[Comment]) -> String {
        let issue_state = match issue.state {
            IssueState::Open => "Open",
            IssueState::Closed => "Closed",
            IssueState::Merged => "Merged",
        };

        let mut summary = String::new();

        summary.push_str(&format!(
            "**Status:** {}\n**Author:** [@{}](https://github.com/{})\n**Created:** {}\n**Updated:** {}\n",
            issue_state,
            issue.author.login,
            issue.author.login,
            issue.created_at.strftime("%Y-%m-%d %H:%M"),
            issue.updated_at.strftime("%Y-%m-%d %H:%M")
        ));

        if !issue.labels.is_empty() {
            let labels: Vec<String> = issue
                .labels
                .iter()
                .map(|l| format!("`{}`", l.name))
                .collect();
            summary.push_str(&format!("**Labels:** {}\n", labels.join(" ")));
        }

        summary.push_str(&format!("**URL:** {}\n\n", issue.url));

        // Add description
        if let Some(body) = &issue.body {
            summary.push_str("## Description\n\n");
            summary.push_str(body);
            summary.push_str("\n\n");
        }

        // Add comments section
        if !comments.is_empty() {
            summary.push_str(&format!("## Comments ({})\n\n", comments.len()));
            for (i, comment) in comments.iter().enumerate() {
                summary.push_str(&format!(
                    "### Comment {} by [@{}](https://github.com/{}) ({})\n\n{}\n\n",
                    i + 1,
                    comment.author.login,
                    comment.author.login,
                    comment.created_at.strftime("%Y-%m-%d %H:%M"),
                    comment.body
                ));
            }
        }

        self.format_final_summary(issue, &summary)
    }

    /// Add title header and footer to summary
    fn format_final_summary(&self, issue: &Issue, content: &str) -> String {
        let issue_type = if issue.is_pull_request { "PR" } else { "Issue" };
        let title_header = format!(
            "# [{} #{}: {}]({})\n\n",
            issue_type, issue.number, issue.title, issue.url
        );

        format!(
            "{}{}\n\n---\n\n*Summary generated by gh-report v{} for [{}]({})*\n",
            title_header,
            content,
            env!("CARGO_PKG_VERSION"),
            issue_type,
            issue.url
        )
    }

    /// Get diff summary for a PR
    fn get_pr_diff_summary(&self, issue: &Issue) -> Result<String> {
        if !issue.is_pull_request {
            return Ok("Not a pull request".to_string());
        }

        // Extract repo name from URL
        // URL format: https://github.com/owner/repo/pull/123
        let repo_name = self.extract_repo_from_url(&issue.url)?;

        // Fetch PR diff
        match self.github_client.fetch_pr_diff(&repo_name, issue.number) {
            Ok(diff) => {
                let mut summary = format!(
                    "Modified {} files. Added {} lines, removed {} lines.",
                    diff.total_files, diff.total_additions, diff.total_deletions
                );

                // Add file type breakdown if we have files
                if !diff.files.is_empty() {
                    let mut file_types = std::collections::HashMap::new();
                    for file in &diff.files {
                        let ext = std::path::Path::new(&file.filename)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("other");
                        *file_types.entry(ext).or_insert(0u32) += 1;
                    }

                    if file_types.len() > 1 {
                        let mut types: Vec<_> = file_types.iter().collect();
                        types.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count desc

                        let type_summary: Vec<String> = types
                            .iter()
                            .take(5) // Show top 5 file types
                            .map(|(ext, count)| format!("{} {} files", count, ext))
                            .collect();

                        summary.push_str(" File types: ");
                        summary.push_str(&type_summary.join(", "));
                        summary.push('.');
                    }
                }

                Ok(summary)
            }
            Err(e) => {
                warn!("Failed to fetch PR diff for {}: {}", issue.url, e);
                Ok(format!("PR diff unavailable: {}", e))
            }
        }
    }

    /// Extract repository name from GitHub URL
    fn extract_repo_from_url(&self, url: &str) -> Result<String> {
        // Expected format: https://github.com/owner/repo/pull/123 or /issues/123
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 5 && parts[2] == "github.com" {
            Ok(format!("{}/{}", parts[3], parts[4]))
        } else {
            Err(anyhow!("Invalid GitHub URL format: {}", url))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, CommentCount, Label, MockGitHub};
    use jiff::Timestamp;

    fn create_test_issue() -> Issue {
        Issue {
            number: 123,
            title: "Test issue for summarization".to_string(),
            body: Some("This is a test issue body with some details.".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "testuser".to_string(),
                user_type: Some("User".to_string()),
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![Label {
                name: "bug".to_string(),
                color: Some("red".to_string()),
                description: None,
            }],
            url: "https://github.com/test/repo/issues/123".to_string(),
            comments: CommentCount { total_count: 1 },
            is_pull_request: false,
        }
    }

    fn create_test_comment() -> Comment {
        Comment {
            id: 1,
            body: "This looks like a valid bug report.".to_string(),
            author: Author {
                login: "reviewer".to_string(),
                user_type: Some("User".to_string()),
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
        }
    }

    #[test]
    fn test_issue_summarizer_creation() {
        let mut mock = MockGitHub::new();
        mock.issues.push(create_test_issue());
        mock.comments.push(create_test_comment());

        let github_client = GitHubClient::Mock(mock);
        let config = Config::default();

        let _summarizer = IssueSummarizer::new(github_client, &config);

        // Should not panic and should have github client
        // Claude client may or may not be available depending on environment
    }

    #[test]
    fn test_basic_summary_generation() {
        let mut mock = MockGitHub::new();
        let issue = create_test_issue();
        let comment = create_test_comment();

        mock.issues.push(issue.clone());
        mock.comments.push(comment.clone());

        let github_client = GitHubClient::Mock(mock);
        let config = Config::default();
        let summarizer = IssueSummarizer::new(github_client, &config);

        let summary = summarizer.generate_basic_summary(&issue, &vec![comment]);

        assert!(summary.contains("# [Issue #123:"));
        assert!(summary.contains("Test issue for summarization"));
        assert!(summary.contains("@testuser"));
        assert!(summary.contains("`bug`"));
        assert!(summary.contains("This looks like a valid bug report"));
        assert!(summary.contains("@reviewer"));
    }
}
