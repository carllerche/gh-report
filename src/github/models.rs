use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Represents a GitHub issue or pull request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Issue {
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub author: Author,
    #[serde(rename = "createdAt")]
    pub created_at: Timestamp,
    #[serde(rename = "updatedAt")]
    pub updated_at: Timestamp,
    pub labels: Vec<Label>,
    pub url: String,
    pub comments: CommentCount,
    #[serde(rename = "isPullRequest")]
    pub is_pull_request: bool,
}

impl Issue {
    /// Extract repository name from the issue URL
    /// URL format: https://github.com/owner/repo/issues/123 or https://github.com/owner/repo/pull/123
    pub fn repository_name(&self) -> Option<String> {
        let url_without_prefix = self.url.strip_prefix("https://github.com/")?;
        let parts: Vec<&str> = url_without_prefix.split('/').collect();

        if parts.len() >= 2 {
            Some(format!("{}/{}", parts[0], parts[1]))
        } else {
            None
        }
    }
}

/// Issue or PR state
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum IssueState {
    Open,
    Closed,
    Merged,
}

/// Author information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Author {
    pub login: String,
    #[serde(rename = "type")]
    pub user_type: Option<String>,
}

/// Label on an issue/PR
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Label {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

/// Comment count information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommentCount {
    #[serde(rename = "totalCount")]
    pub total_count: u32,
}

/// Represents a comment on an issue or PR
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub author: Author,
    #[serde(rename = "createdAt")]
    pub created_at: Timestamp,
    #[serde(rename = "updatedAt")]
    pub updated_at: Timestamp,
}

/// Repository information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repository {
    pub name: String,
    pub owner: Owner,
    #[serde(rename = "nameWithOwner")]
    pub full_name: String,
    pub description: Option<String>,
    #[serde(rename = "isPrivate")]
    pub is_private: bool,
    #[serde(rename = "isArchived")]
    pub is_archived: bool,
    #[serde(rename = "pushedAt")]
    pub pushed_at: Option<Timestamp>,
    #[serde(rename = "defaultBranchRef")]
    pub default_branch: Option<BranchRef>,
}

/// Repository owner
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Owner {
    pub login: String,
}

/// Branch reference
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BranchRef {
    pub name: String,
}

/// Notification/mention
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Notification {
    pub id: String,
    pub unread: bool,
    pub reason: String,
    pub updated_at: Timestamp,
    pub repository: NotificationRepo,
    pub subject: NotificationSubject,
}

/// Repository info in notification
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationRepo {
    pub full_name: String,
}

/// Notification subject
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationSubject {
    pub title: String,
    #[serde(rename = "type")]
    pub subject_type: String,
    pub url: Option<String>,
}

/// Context for an issue/PR (cached)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueContext {
    pub issue_number: u32,
    pub repo: String,
    pub last_updated: Timestamp,
    pub summary: String,
    pub key_points: Vec<String>,
    pub participants: Vec<String>,
    pub last_processed_comment_id: Option<u64>,
}

/// Repository status for tracking
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum RepoStatus {
    Active,
    Deleted,
    Inaccessible,
}

/// Activity summary for a repository
#[derive(Debug, Default)]
pub struct RepoActivity {
    pub new_issues: Vec<Issue>,
    pub new_prs: Vec<Issue>,
    pub updated_issues: Vec<Issue>,
    pub updated_prs: Vec<Issue>,
    pub merged_prs: Vec<Issue>,
    pub closed_issues: Vec<Issue>,
    pub new_comments: Vec<(Issue, Vec<Comment>)>,
}

/// REST API Issue representation (for deserialization from gh api)
#[derive(Debug, Clone, Deserialize)]
pub struct RestIssue {
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: RestUser,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub labels: Vec<Label>,
    pub html_url: String,
    pub comments: u32,
    pub pull_request: Option<serde_json::Value>,
    // PR-specific fields for detecting merge status
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub merged_at: Option<Timestamp>,
    // Additional fields that might be present
    #[serde(default)]
    pub sub_issues_summary: Option<serde_json::Value>,
    #[serde(default)]
    pub issue_dependencies_summary: Option<serde_json::Value>,
    #[serde(default)]
    pub state_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RestUser {
    pub login: String,
    #[serde(rename = "type")]
    pub user_type: Option<String>,
}

impl From<RestIssue> for Issue {
    fn from(rest: RestIssue) -> Self {
        Issue {
            number: rest.number,
            title: rest.title,
            body: rest.body,
            state: match rest.state.as_str() {
                "open" => IssueState::Open,
                "closed" => {
                    // For PRs, check if it was merged
                    if rest.pull_request.is_some() && rest.merged.unwrap_or(false) {
                        IssueState::Merged
                    } else {
                        IssueState::Closed
                    }
                }
                _ => IssueState::Closed,
            },
            author: Author {
                login: rest.user.login,
                user_type: rest.user.user_type,
            },
            created_at: rest.created_at,
            updated_at: rest.updated_at,
            labels: rest.labels,
            url: rest.html_url,
            comments: CommentCount {
                total_count: rest.comments,
            },
            is_pull_request: rest.pull_request.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_serialization() {
        let issue = Issue {
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
            labels: vec![Label {
                name: "bug".to_string(),
                color: Some("red".to_string()),
                description: Some("Bug report".to_string()),
            }],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 5 },
            is_pull_request: false,
        };

        // Test serialization
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("\"number\":42"));
        assert!(json.contains("\"title\":\"Test Issue\""));

        // Test deserialization
        let issue2: Issue = serde_json::from_str(&json).unwrap();
        assert_eq!(issue.number, issue2.number);
        assert_eq!(issue.title, issue2.title);
    }

    #[test]
    fn test_issue_state() {
        let states = vec![IssueState::Open, IssueState::Closed, IssueState::Merged];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let state2: IssueState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, state2);
        }
    }

    #[test]
    fn test_repo_activity_default() {
        let activity = RepoActivity::default();

        assert!(activity.new_issues.is_empty());
        assert!(activity.updated_issues.is_empty());
        assert!(activity.new_prs.is_empty());
        assert!(activity.updated_prs.is_empty());
        assert!(activity.new_comments.is_empty());
    }

    #[test]
    fn test_comment_serialization() {
        let comment = Comment {
            id: 12345,
            author: Author {
                login: "commenter".to_string(),
                user_type: None,
            },
            body: "This is a comment".to_string(),
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
        };

        let json = serde_json::to_string(&comment).unwrap();
        let comment2: Comment = serde_json::from_str(&json).unwrap();

        assert_eq!(comment.id, comment2.id);
        assert_eq!(comment.body, comment2.body);
        assert_eq!(comment.author.login, comment2.author.login);
    }

    #[test]
    fn test_repo_status() {
        assert_eq!(RepoStatus::Active, RepoStatus::Active);
        assert_ne!(RepoStatus::Active, RepoStatus::Deleted);

        // Test serialization
        let json = serde_json::to_string(&RepoStatus::Inaccessible).unwrap();
        let status: RepoStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, RepoStatus::Inaccessible);
    }
}

/// PR file change information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrFileChange {
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
    pub patch: Option<String>,
}

/// PR diff information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrDiff {
    pub files: Vec<PrFileChange>,
    pub total_additions: u32,
    pub total_deletions: u32,
    pub total_files: u32,
}

/// GitHub activity event
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActivityEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub actor: Author,
    pub repo: ActivityRepo,
    pub payload: serde_json::Value,
    #[serde(rename = "created_at")]
    pub created_at: Timestamp,
    #[serde(rename = "public")]
    pub is_public: bool,
}

/// Repository information in activity events
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActivityRepo {
    pub id: u64,
    pub name: String,
    pub url: String,
}
