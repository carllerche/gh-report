use serde::{Deserialize, Serialize};
use jiff::Timestamp;

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
    pub new_comments: Vec<(Issue, Vec<Comment>)>,
}