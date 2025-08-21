use crate::github::models::*;
use anyhow::{anyhow, Context, Result};
use jiff::{Timestamp, ToSpan};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::process::Command;

/// GitHub client abstraction
pub enum GitHubClient {
    Real(RealGitHub),
    #[cfg(test)]
    Mock(MockGitHub),
}

impl GitHubClient {
    /// Create a new real GitHub client
    pub fn new() -> Result<Self> {
        Ok(GitHubClient::Real(RealGitHub::new()?))
    }

    /// Create a mock client for testing
    #[cfg(test)]
    pub fn mock() -> Self {
        GitHubClient::Mock(MockGitHub::new())
    }

    /// Fetch issues and PRs for a repository
    pub fn fetch_issues(&self, repo: &str, since: Option<Timestamp>) -> Result<Vec<Issue>> {
        match self {
            GitHubClient::Real(client) => client.fetch_issues(repo, since),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_issues(repo, since),
        }
    }

    /// Fetch comments for an issue/PR
    pub fn fetch_comments(
        &self,
        repo: &str,
        issue_number: u32,
        since: Option<Timestamp>,
    ) -> Result<Vec<Comment>> {
        match self {
            GitHubClient::Real(client) => client.fetch_comments(repo, issue_number, since),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_comments(repo, issue_number, since),
        }
    }

    /// Fetch repository information
    pub fn fetch_repository(&self, repo: &str) -> Result<Repository> {
        match self {
            GitHubClient::Real(client) => client.fetch_repository(repo),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_repository(repo),
        }
    }

    /// Search for mentions of the current user
    pub fn fetch_mentions(&self, since: Timestamp) -> Result<Vec<Issue>> {
        match self {
            GitHubClient::Real(client) => client.fetch_mentions(since),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_mentions(since),
        }
    }

    /// Get current authenticated user
    pub fn get_current_user(&self) -> Result<String> {
        match self {
            GitHubClient::Real(client) => client.get_current_user(),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.get_current_user(),
        }
    }

    /// Fetch a single issue or PR with its comments
    pub fn fetch_single_issue(
        &self,
        repo: &str,
        issue_number: u32,
    ) -> Result<(Issue, Vec<Comment>)> {
        match self {
            GitHubClient::Real(client) => client.fetch_single_issue(repo, issue_number),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_single_issue(repo, issue_number),
        }
    }

    /// Fetch PR diff/file changes for a pull request
    pub fn fetch_pr_diff(&self, repo: &str, pr_number: u32) -> Result<PrDiff> {
        match self {
            GitHubClient::Real(client) => client.fetch_pr_diff(repo, pr_number),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_pr_diff(repo, pr_number),
        }
    }

    /// Fetch user's activity events
    pub fn fetch_activity(&self, days: u32) -> Result<Vec<ActivityEvent>> {
        match self {
            GitHubClient::Real(client) => client.fetch_activity(days),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_activity(days),
        }
    }
}

/// Real GitHub client using gh CLI
pub struct RealGitHub {
    gh_path: PathBuf,
}

impl RealGitHub {
    /// Create a new real GitHub client
    pub fn new() -> Result<Self> {
        // Check if gh exists
        let gh_path = which_gh()?;

        // Verify version
        crate::github::check_gh_version()?;

        Ok(RealGitHub { gh_path })
    }

    /// Execute a gh command and parse JSON output
    fn execute_gh<T: DeserializeOwned>(&self, args: &[&str]) -> Result<T> {
        let output = Command::new(&self.gh_path)
            .args(args)
            .output()
            .context("Failed to execute gh command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check for specific error conditions
            if stderr.contains("404") || stderr.contains("not found") {
                return Err(anyhow!("Resource not found"));
            }
            if stderr.contains("401") || stderr.contains("403") {
                return Err(anyhow!("Authentication failed. Run 'gh auth login'"));
            }

            return Err(anyhow!("gh command failed: {}", stderr));
        }

        let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 in gh output")?;

        serde_json::from_str(&stdout).context("Failed to parse gh JSON output")
    }

    /// Execute gh and return raw string output
    fn execute_gh_raw(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.gh_path)
            .args(args)
            .output()
            .context("Failed to execute gh command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("gh command failed: {}", stderr));
        }

        String::from_utf8(output.stdout).context("Invalid UTF-8 in gh output")
    }

    /// Fetch issues and PRs for a repository
    pub fn fetch_issues(&self, repo: &str, since: Option<Timestamp>) -> Result<Vec<Issue>> {
        use crate::github::models::RestIssue;

        // Build endpoint with query parameters
        let endpoint = if let Some(since_ts) = since {
            format!("repos/{}/issues?since={}", repo, since_ts.to_string())
        } else {
            format!("repos/{}/issues", repo)
        };

        let args = vec!["api", &endpoint, "--paginate"];

        // Deserialize as RestIssue and convert to Issue
        let rest_issues: Vec<RestIssue> = self.execute_gh(&args)?;
        Ok(rest_issues.into_iter().map(Into::into).collect())
    }

    /// Fetch comments for an issue/PR
    pub fn fetch_comments(
        &self,
        repo: &str,
        issue_number: u32,
        since: Option<Timestamp>,
    ) -> Result<Vec<Comment>> {
        // Build endpoint with query parameters
        let endpoint = if let Some(since_ts) = since {
            format!(
                "repos/{}/issues/{}/comments?since={}",
                repo,
                issue_number,
                since_ts.to_string()
            )
        } else {
            format!("repos/{}/issues/{}/comments", repo, issue_number)
        };

        let args = vec!["api", &endpoint, "--paginate"];

        self.execute_gh(&args)
    }

    /// Fetch repository information
    pub fn fetch_repository(&self, repo: &str) -> Result<Repository> {
        let endpoint = format!("repos/{}", repo);
        let args = vec!["api", &endpoint];

        self.execute_gh(&args)
    }

    /// Search for mentions of the current user
    pub fn fetch_mentions(&self, since: Timestamp) -> Result<Vec<Issue>> {
        let query = format!("involves:@me updated:>{}", since.strftime("%Y-%m-%d"));
        // URL encode the query parameter
        let encoded_query = query
            .replace(" ", "%20")
            .replace(":", "%3A")
            .replace(">", "%3E");
        let endpoint = format!("search/issues?q={}", encoded_query);
        let args = vec!["api", &endpoint];

        #[derive(serde::Deserialize)]
        struct SearchResult {
            items: Vec<Issue>,
        }

        let result: SearchResult = self.execute_gh(&args)?;
        Ok(result.items)
    }

    /// Get current authenticated user
    pub fn get_current_user(&self) -> Result<String> {
        let output = self.execute_gh_raw(&["api", "user"])?;

        #[derive(serde::Deserialize)]
        struct User {
            login: String,
        }

        let user: User = serde_json::from_str(&output)?;
        Ok(user.login)
    }

    /// Fetch a single issue or PR with its comments
    pub fn fetch_single_issue(
        &self,
        repo: &str,
        issue_number: u32,
    ) -> Result<(Issue, Vec<Comment>)> {
        use crate::github::models::RestIssue;

        // First, fetch the issue/PR details
        let issue_endpoint = format!("repos/{}/issues/{}", repo, issue_number);
        let issue_args = vec!["api", &issue_endpoint];

        let rest_issue: RestIssue = self.execute_gh(&issue_args)?;
        let issue: Issue = rest_issue.into();

        // Then fetch all comments
        let comments_endpoint = format!("repos/{}/issues/{}/comments", repo, issue_number);
        let comments_args = vec!["api", &comments_endpoint, "--paginate"];

        let comments: Vec<Comment> = self.execute_gh(&comments_args)?;

        Ok((issue, comments))
    }

    /// Fetch PR diff/file changes for a pull request
    pub fn fetch_pr_diff(&self, repo: &str, pr_number: u32) -> Result<PrDiff> {
        // Fetch PR files endpoint which gives us the diff data
        let endpoint = format!("repos/{}/pulls/{}/files", repo, pr_number);
        let args = vec!["api", &endpoint, "--paginate"];

        let files: Vec<PrFileChange> = self.execute_gh(&args)?;

        // Calculate totals
        let total_additions = files.iter().map(|f| f.additions).sum();
        let total_deletions = files.iter().map(|f| f.deletions).sum();
        let total_files = files.len() as u32;

        Ok(PrDiff {
            files,
            total_additions,
            total_deletions,
            total_files,
        })
    }

    /// Fetch user's activity events (received events for subscribed repos)
    pub fn fetch_activity(&self, days: u32) -> Result<Vec<ActivityEvent>> {
        // Get current username first
        let username = self.get_current_user()?;
        
        // Use gh api to fetch received events (activities on subscribed repos)
        let endpoint = format!("/users/{}/received_events", username);
        let args = vec![
            "api",
            &endpoint,
            "--paginate"
        ];

        let events: Vec<ActivityEvent> = self.execute_gh(&args)?;

        // Filter by date - only include events from the last N days
        let cutoff = jiff::Timestamp::now() - (days as i64 * 24).hours();
        let filtered_events: Vec<ActivityEvent> = events
            .into_iter()
            .filter(|event| event.created_at >= cutoff)
            .collect();

        Ok(filtered_events)
    }
}

/// Find gh executable path
fn which_gh() -> Result<PathBuf> {
    // Try common locations first
    let common_paths = [
        "/usr/local/bin/gh",
        "/usr/bin/gh",
        "/opt/homebrew/bin/gh",
        "/home/linuxbrew/.linuxbrew/bin/gh",
    ];

    for path in &common_paths {
        let path = Path::new(path);
        if path.exists() {
            return Ok(path.to_path_buf());
        }
    }

    // Fall back to using 'which' command
    let output = Command::new("which")
        .arg("gh")
        .output()
        .context("Failed to run 'which gh'")?;

    if output.status.success() {
        let path = String::from_utf8(output.stdout)?.trim().to_string();
        return Ok(PathBuf::from(path));
    }

    Err(anyhow!(
        "GitHub CLI (gh) not found. Please install it from https://cli.github.com/"
    ))
}

/// Mock GitHub client for testing
#[cfg(test)]
pub struct MockGitHub {
    pub issues: Vec<Issue>,
    pub comments: Vec<Comment>,
    pub repositories: Vec<Repository>,
    pub current_user: String,
    pub pr_diffs: Vec<(u32, PrDiff)>, // (pr_number, diff)
}

#[cfg(test)]
impl MockGitHub {
    pub fn new() -> Self {
        MockGitHub {
            issues: vec![],
            comments: vec![],
            repositories: vec![],
            current_user: "testuser".to_string(),
            pr_diffs: vec![],
        }
    }

    pub fn fetch_issues(&self, _repo: &str, _since: Option<Timestamp>) -> Result<Vec<Issue>> {
        Ok(self.issues.clone())
    }

    pub fn fetch_comments(
        &self,
        _repo: &str,
        _issue_number: u32,
        _since: Option<Timestamp>,
    ) -> Result<Vec<Comment>> {
        Ok(self.comments.clone())
    }

    pub fn fetch_repository(&self, repo: &str) -> Result<Repository> {
        self.repositories
            .iter()
            .find(|r| r.full_name == repo)
            .cloned()
            .ok_or_else(|| anyhow!("Repository not found"))
    }

    pub fn fetch_mentions(&self, _since: Timestamp) -> Result<Vec<Issue>> {
        Ok(self.issues.clone())
    }

    pub fn get_current_user(&self) -> Result<String> {
        Ok(self.current_user.clone())
    }

    pub fn fetch_single_issue(
        &self,
        _repo: &str,
        issue_number: u32,
    ) -> Result<(Issue, Vec<Comment>)> {
        // Find the issue by number
        let issue = self
            .issues
            .iter()
            .find(|i| i.number == issue_number)
            .cloned()
            .ok_or_else(|| anyhow!("Issue #{} not found", issue_number))?;

        // Return issue with all comments (mock doesn't filter by issue)
        Ok((issue, self.comments.clone()))
    }

    pub fn fetch_pr_diff(&self, _repo: &str, pr_number: u32) -> Result<PrDiff> {
        // Find the PR diff by number
        self.pr_diffs
            .iter()
            .find(|(num, _)| *num == pr_number)
            .map(|(_, diff)| diff.clone())
            .ok_or_else(|| anyhow!("PR #{} diff not found", pr_number))
    }

    pub fn fetch_activity(&self, _days: u32) -> Result<Vec<ActivityEvent>> {
        // Return empty activity for mock
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::IssueState;

    #[test]
    fn test_mock_github_client() {
        // Create mock client with test data
        let mut mock = MockGitHub::new();

        // Add test issue
        mock.issues.push(Issue {
            number: 42,
            title: "Test Issue".to_string(),
            body: Some("Test body".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "testuser".to_string(),
                user_type: Some("User".to_string()),
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        });

        // Create client
        let client = GitHubClient::Mock(mock);

        // Test fetching issues
        let issues = client.fetch_issues("test/repo", None).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 42);
    }

    #[test]
    fn test_mock_current_user() {
        let mock = MockGitHub::new();
        let client = GitHubClient::Mock(mock);

        let user = client.get_current_user().unwrap();
        assert_eq!(user, "testuser");
    }

    #[test]
    fn test_fetch_single_issue() {
        let mut mock = MockGitHub::new();

        // Add test issue
        mock.issues.push(Issue {
            number: 123,
            title: "Test Issue for Single Fetch".to_string(),
            body: Some("Detailed issue description".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "issueauthor".to_string(),
                user_type: Some("User".to_string()),
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![],
            url: "https://github.com/test/repo/issues/123".to_string(),
            comments: CommentCount { total_count: 2 },
            is_pull_request: false,
        });

        // Add test comments
        mock.comments.push(Comment {
            id: 1,
            body: "First comment".to_string(),
            author: Author {
                login: "commenter1".to_string(),
                user_type: Some("User".to_string()),
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
        });

        mock.comments.push(Comment {
            id: 2,
            body: "Second comment".to_string(),
            author: Author {
                login: "commenter2".to_string(),
                user_type: Some("User".to_string()),
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
        });

        let client = GitHubClient::Mock(mock);

        // Test fetching single issue
        let (issue, comments) = client.fetch_single_issue("test/repo", 123).unwrap();

        assert_eq!(issue.number, 123);
        assert_eq!(issue.title, "Test Issue for Single Fetch");
        assert_eq!(issue.body, Some("Detailed issue description".to_string()));
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].body, "First comment");
        assert_eq!(comments[1].body, "Second comment");
    }

    #[test]
    fn test_fetch_single_issue_not_found() {
        let mock = MockGitHub::new();
        let client = GitHubClient::Mock(mock);

        // Test fetching non-existent issue
        let result = client.fetch_single_issue("test/repo", 999);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Issue #999 not found"));
    }
}
