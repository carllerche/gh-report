use gh_daily_report::github::{Issue, Comment};

// Integration tests focus on data model serialization/deserialization
// Mock tests are better as unit tests inside the module

#[test]
fn test_fixture_deserialization() {
    // Test that our fixture files can be properly deserialized
    let issues_json = include_str!("../fixtures/github/issues.json");
    let issues: Vec<Issue> = serde_json::from_str(issues_json).unwrap();
    
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].number, 123);
    assert_eq!(issues[0].title, "Add support for async trait methods");
    assert!(!issues[0].is_pull_request);
    assert!(issues[1].is_pull_request);
    
    let comments_json = include_str!("../fixtures/github/comments.json");
    let comments: Vec<Comment> = serde_json::from_str(comments_json).unwrap();
    
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].id, 1001);
}