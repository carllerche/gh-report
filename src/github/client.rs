use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use jiff::Timestamp;
use serde::de::DeserializeOwned;
use crate::github::models::*;

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

    /// Fetch issues and PRs for a repository
    pub fn fetch_issues(&self, repo: &str, since: Option<Timestamp>) -> Result<Vec<Issue>> {
        match self {
            GitHubClient::Real(client) => client.fetch_issues(repo, since),
            #[cfg(test)]
            GitHubClient::Mock(client) => client.fetch_issues(repo, since),
        }
    }

    /// Fetch comments for an issue/PR
    pub fn fetch_comments(&self, repo: &str, issue_number: u32, since: Option<Timestamp>) -> Result<Vec<Comment>> {
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

        let stdout = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in gh output")?;
        
        serde_json::from_str(&stdout)
            .context("Failed to parse gh JSON output")
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

        String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in gh output")
    }

    /// Fetch issues and PRs for a repository
    pub fn fetch_issues(&self, repo: &str, since: Option<Timestamp>) -> Result<Vec<Issue>> {
        let endpoint = format!("repos/{}/issues", repo);
        let mut args = vec![
            "api",
            &endpoint,
            "--paginate",
            "--json",
            "number,title,body,state,author,createdAt,updatedAt,labels,url,comments,isPullRequest",
        ];

        // Add since parameter if provided
        let since_param;
        if let Some(since_ts) = since {
            since_param = format!("since={}", since_ts.to_string());
            args.push("--field");
            args.push(&since_param);
        }

        self.execute_gh(&args)
    }

    /// Fetch comments for an issue/PR
    pub fn fetch_comments(&self, repo: &str, issue_number: u32, since: Option<Timestamp>) -> Result<Vec<Comment>> {
        let endpoint = format!("repos/{}/issues/{}/comments", repo, issue_number);
        let mut args = vec![
            "api",
            &endpoint,
            "--paginate",
            "--json",
            "id,body,author,createdAt,updatedAt",
        ];

        // Add since parameter if provided
        let since_param;
        if let Some(since_ts) = since {
            since_param = format!("since={}", since_ts.to_string());
            args.push("--field");
            args.push(&since_param);
        }

        self.execute_gh(&args)
    }

    /// Fetch repository information
    pub fn fetch_repository(&self, repo: &str) -> Result<Repository> {
        let endpoint = format!("repos/{}", repo);
        let args = vec![
            "api",
            &endpoint,
            "--json",
            "name,owner,nameWithOwner,description,isPrivate,isArchived,pushedAt,defaultBranchRef",
        ];

        self.execute_gh(&args)
    }

    /// Search for mentions of the current user
    pub fn fetch_mentions(&self, since: Timestamp) -> Result<Vec<Issue>> {
        let query = format!("involves:@me updated:>{}", since.strftime("%Y-%m-%d"));
        let field_param = format!("q={}", query);
        let args = vec![
            "api",
            "search/issues",
            "--field",
            &field_param,
            "--json",
            "items",
        ];

        #[derive(serde::Deserialize)]
        struct SearchResult {
            items: Vec<Issue>,
        }

        let result: SearchResult = self.execute_gh(&args)?;
        Ok(result.items)
    }

    /// Get current authenticated user
    pub fn get_current_user(&self) -> Result<String> {
        let output = self.execute_gh_raw(&["api", "user", "--json", "login"])?;
        
        #[derive(serde::Deserialize)]
        struct User {
            login: String,
        }
        
        let user: User = serde_json::from_str(&output)?;
        Ok(user.login)
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
        let path = String::from_utf8(output.stdout)?
            .trim()
            .to_string();
        return Ok(PathBuf::from(path));
    }

    Err(anyhow!("GitHub CLI (gh) not found. Please install it from https://cli.github.com/"))
}

/// Mock GitHub client for testing
#[cfg(test)]
pub struct MockGitHub {
    pub issues: Vec<Issue>,
    pub comments: Vec<Comment>,
    pub repositories: Vec<Repository>,
    pub current_user: String,
}

#[cfg(test)]
impl MockGitHub {
    pub fn new() -> Self {
        MockGitHub {
            issues: vec![],
            comments: vec![],
            repositories: vec![],
            current_user: "testuser".to_string(),
        }
    }

    pub fn fetch_issues(&self, _repo: &str, _since: Option<Timestamp>) -> Result<Vec<Issue>> {
        Ok(self.issues.clone())
    }

    pub fn fetch_comments(&self, _repo: &str, _issue_number: u32, _since: Option<Timestamp>) -> Result<Vec<Comment>> {
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
}