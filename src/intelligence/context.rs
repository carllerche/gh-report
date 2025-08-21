use crate::config::Importance;
use crate::intelligence::{ActionItem, PrioritizedIssue, Urgency};

/// Build simple context prompt for AI summarization
pub fn build_context_prompt() -> String {
    r#"## Summarization Guidelines

When summarizing GitHub activity:
1. Prioritize security issues, breaking changes, and critical bugs first
2. Highlight pull requests that need review
3. Group related items together for clarity
4. For each high-priority item, explain why it matters
5. Suggest specific actions when appropriate
6. Keep summaries concise but informative
"#
    .to_string()
}

/// Extract action items from prioritized issues
pub fn extract_action_items(prioritized_issues: &[PrioritizedIssue]) -> Vec<ActionItem> {
    let mut action_items = Vec::new();

    for issue in prioritized_issues {
        // Determine urgency based on score and labels
        let urgency = determine_urgency(issue);

        // Generate action description
        let action = generate_action(&issue.issue);

        if let Some(description) = action {
            let reason = generate_reason(issue);

            action_items.push(ActionItem {
                description,
                issue: issue.issue.clone(),
                repo: issue.repo.clone(),
                urgency,
                reason,
            });
        }
    }

    // Sort by urgency (highest first)
    action_items.sort_by(|a, b| b.urgency.cmp(&a.urgency));

    // Limit to top 10 action items
    action_items.truncate(10);

    action_items
}

/// Determine urgency level for an issue
fn determine_urgency(issue: &PrioritizedIssue) -> Urgency {
    // Check for critical indicators based on labels
    if issue.issue.labels.iter().any(|l| {
        let name = l.name.to_lowercase();
        name.contains("security") || name.contains("critical")
    }) {
        return Urgency::Critical;
    }

    if issue.importance == Importance::Critical && issue.score.total > 80 {
        return Urgency::Critical;
    }

    // Check for high urgency based on labels and score
    if issue.issue.labels.iter().any(|l| {
        let name = l.name.to_lowercase();
        name.contains("breaking") || name.contains("bug") || name.contains("urgent")
    }) {
        return Urgency::High;
    }

    if issue.score.total > 60 {
        return Urgency::High;
    }

    // Check for medium urgency
    if issue.score.total > 30 {
        return Urgency::Medium;
    }

    Urgency::Low
}

/// Generate action description for an issue
fn generate_action(issue: &crate::github::Issue) -> Option<String> {
    // Skip generating actions for closed or merged items
    match issue.state {
        crate::github::IssueState::Closed | crate::github::IssueState::Merged => {
            return None;
        }
        crate::github::IssueState::Open => {
            // Continue to generate actions for open items
        }
    }

    // Security issues based on labels
    if issue.labels.iter().any(|l| {
        let name = l.name.to_lowercase();
        name.contains("security") || name.contains("critical")
    }) {
        return Some(format!(
            "Review and address security {} #{}",
            if issue.is_pull_request { "PR" } else { "issue" },
            issue.number
        ));
    }

    // Breaking changes based on labels
    if issue.labels.iter().any(|l| {
        let name = l.name.to_lowercase();
        name.contains("breaking")
    }) {
        return Some(format!(
            "Review breaking change in {} #{}",
            if issue.is_pull_request { "PR" } else { "issue" },
            issue.number
        ));
    }

    // High comment activity
    if issue.comments.total_count > 10 {
        return Some(format!(
            "Check active discussion on {} #{}",
            if issue.is_pull_request { "PR" } else { "issue" },
            issue.number
        ));
    }

    // New critical/high importance items
    if issue.is_pull_request {
        Some(format!("Review [PR #{}]({})", issue.number, issue.url))
    } else if issue
        .labels
        .iter()
        .any(|l| l.name.to_lowercase().contains("bug") || l.name.to_lowercase().contains("urgent"))
    {
        Some(format!("Address [issue #{}]({})", issue.number, issue.url))
    } else {
        None
    }
}

/// Generate reason for action item
fn generate_reason(issue: &PrioritizedIssue) -> String {
    let mut reasons = Vec::new();

    // Add label-based reasons
    for label in &issue.issue.labels {
        let name = label.name.to_lowercase();
        if name.contains("security") {
            reasons.push("Security concern".to_string());
        } else if name.contains("breaking") {
            reasons.push("Breaking change".to_string());
        } else if name.contains("bug") || name.contains("urgent") {
            reasons.push("Urgent issue".to_string());
        } else if name.contains("feature") || name.contains("enhancement") {
            reasons.push("New feature".to_string());
        }
    }

    // Add importance reason
    if issue.importance == Importance::Critical {
        reasons.push("Critical repository".to_string());
    } else if issue.importance == Importance::High {
        reasons.push("High priority repository".to_string());
    }

    // Add activity reason
    if issue.issue.comments.total_count > 10 {
        reasons.push(format!("{} comments", issue.issue.comments.total_count));
    }

    if reasons.is_empty() {
        "Requires attention".to_string()
    } else {
        reasons.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, CommentCount, Issue, IssueState, Label as GHLabel};
    use crate::intelligence::PriorityScore;
    use jiff::Timestamp;

    #[test]
    fn test_build_context_prompt() {
        let prompt = build_context_prompt();
        assert!(prompt.contains("Summarization Guidelines"));
        assert!(prompt.contains("security issues"));
    }

    #[test]
    fn test_extract_action_items() {
        let issue = Issue {
            number: 42,
            title: "Security fix".to_string(),
            body: None,
            state: IssueState::Open,
            author: Author {
                login: "user".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![GHLabel {
                name: "security".to_string(),
                color: Some("red".to_string()),
                description: None,
            }],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        };

        let prioritized = vec![PrioritizedIssue {
            issue: issue.clone(),
            repo: "test/repo".to_string(),
            score: PriorityScore {
                total: 90,
                importance_score: 30,
                recency_score: 30,
                activity_score: 0,
                rule_match_score: 30,
                label_score: 0,
            },
            importance: Importance::High,
        }];

        let actions = extract_action_items(&prioritized);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].urgency, Urgency::Critical);
        assert!(actions[0].description.contains("security"));
        assert!(actions[0].reason.contains("Security concern"));
    }

    #[test]
    fn test_urgency_determination() {
        let issue = Issue {
            number: 100,
            title: "Test".to_string(),
            body: None,
            state: IssueState::Open,
            author: Author {
                login: "user".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![],
            url: "https://github.com/test/repo/pull/100".to_string(),
            comments: CommentCount { total_count: 15 },
            is_pull_request: true,
        };

        let prioritized = PrioritizedIssue {
            issue,
            repo: "test/repo".to_string(),
            score: PriorityScore {
                total: 70,
                importance_score: 20,
                recency_score: 20,
                activity_score: 20,
                rule_match_score: 0,
                label_score: 10,
            },
            importance: Importance::Medium,
        };

        let urgency = determine_urgency(&prioritized);
        assert_eq!(urgency, Urgency::High);
    }
}
