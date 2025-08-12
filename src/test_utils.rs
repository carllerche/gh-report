//! Test utilities for gh-daily-report
#![cfg(test)]

use crate::github::{GitHubClient, MockGitHub, Issue, IssueState, Author, CommentCount, Label};
use jiff::Timestamp;

/// Create a mock GitHub client with test data
pub fn create_test_github_client() -> GitHubClient {
    let mut mock = MockGitHub::new();
    
    // Add some test issues
    mock.issues.push(create_test_issue(1, "Test Issue 1", false));
    mock.issues.push(create_test_issue(2, "Test PR 1", true));
    
    GitHubClient::Mock(mock)
}

/// Create a test issue
pub fn create_test_issue(number: u32, title: &str, is_pr: bool) -> Issue {
    Issue {
        number,
        title: title.to_string(),
        body: Some(format!("Body of {}", title)),
        state: IssueState::Open,
        author: Author {
            login: "testuser".to_string(),
            user_type: Some("User".to_string()),
        },
        created_at: Timestamp::now(),
        updated_at: Timestamp::now(),
        labels: vec![],
        url: format!("https://github.com/test/repo/{}/{}", 
            if is_pr { "pull" } else { "issues" }, number),
        comments: CommentCount { total_count: 0 },
        is_pull_request: is_pr,
    }
}

/// Create a test issue with labels
pub fn create_test_issue_with_labels(number: u32, title: &str, labels: Vec<&str>) -> Issue {
    let mut issue = create_test_issue(number, title, false);
    issue.labels = labels.into_iter().map(|name| Label {
        name: name.to_string(),
        color: Some("blue".to_string()),
        description: None,
    }).collect();
    issue
}