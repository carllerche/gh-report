use crate::config::{Importance, Label};
use crate::intelligence::{ActionItem, PrioritizedIssue, Urgency};
use std::collections::HashMap;

/// Build context prompt for AI summarization
pub fn build_context_prompt(
    labels: &[Label],
    repo_importances: &HashMap<String, Importance>,
) -> String {
    let mut prompt = String::new();

    // Add label contexts
    if !labels.is_empty() {
        prompt.push_str("## User Context for Labels\n\n");

        for label in labels {
            prompt.push_str(&format!(
                "### {} ({})\n",
                label.name,
                format_importance(label.importance)
            ));
            prompt.push_str(&format!("{}\n", label.context));

            if !label.watch_rules.is_empty() {
                prompt.push_str("Watch rules: ");
                prompt.push_str(&label.watch_rules.join(", "));
                prompt.push_str("\n");
            }

            prompt.push_str("\n");
        }
    }

    // Add repository importance context
    if !repo_importances.is_empty() {
        prompt.push_str("## Repository Importance Levels\n\n");

        let mut by_importance: HashMap<Importance, Vec<String>> = HashMap::new();
        for (repo, importance) in repo_importances {
            by_importance
                .entry(*importance)
                .or_insert_with(Vec::new)
                .push(repo.clone());
        }

        for importance in [
            Importance::Critical,
            Importance::High,
            Importance::Medium,
            Importance::Low,
        ] {
            if let Some(repos) = by_importance.get(&importance) {
                prompt.push_str(&format!(
                    "- {}: {}\n",
                    format_importance(importance),
                    repos.join(", ")
                ));
            }
        }
        prompt.push_str("\n");
    }

    // Add general instructions
    prompt.push_str(
        r#"## Summarization Guidelines

When summarizing GitHub activity:
1. Prioritize items based on the importance levels and context provided
2. Highlight security issues, breaking changes, and critical bugs first
3. Group related items together for clarity
4. For each high-priority item, explain why it matters based on the context
5. Suggest specific actions when appropriate
6. Keep summaries concise but informative
"#,
    );

    prompt
}

/// Extract action items from prioritized issues
pub fn extract_action_items(prioritized_issues: &[PrioritizedIssue]) -> Vec<ActionItem> {
    let mut action_items = Vec::new();

    for issue in prioritized_issues {
        // Determine urgency based on score and matched rules
        let urgency = determine_urgency(issue);

        // Generate action description
        let action = generate_action(&issue.issue, &issue.matched_rules);

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
    // Check for critical indicators
    if issue
        .matched_rules
        .iter()
        .any(|r| r.rule_type == "security_issues")
    {
        return Urgency::Critical;
    }

    if issue.importance == Importance::Critical && issue.score.total > 80 {
        return Urgency::Critical;
    }

    // Check for high urgency
    if issue
        .matched_rules
        .iter()
        .any(|r| r.rule_type == "breaking_changes" || r.rule_type == "review_requests")
    {
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
fn generate_action(
    issue: &crate::github::Issue,
    matched_rules: &[crate::intelligence::MatchedRule],
) -> Option<String> {
    // Security issues
    if matched_rules
        .iter()
        .any(|r| r.rule_type == "security_issues")
    {
        return Some(format!(
            "Review and address security {} #{}",
            if issue.is_pull_request { "PR" } else { "issue" },
            issue.number
        ));
    }

    // Review requests
    if matched_rules
        .iter()
        .any(|r| r.rule_type == "review_requests")
    {
        return Some(format!("Review [PR #{}]({})", issue.number, issue.url));
    }

    // Breaking changes
    if matched_rules
        .iter()
        .any(|r| r.rule_type == "breaking_changes")
    {
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

    // Add matched rules
    for rule in &issue.matched_rules {
        match rule.rule_type.as_str() {
            "security_issues" => reasons.push("Security concern".to_string()),
            "breaking_changes" => reasons.push("Breaking change".to_string()),
            "review_requests" => reasons.push("Review requested".to_string()),
            "api_changes" => reasons.push("API change".to_string()),
            "performance" => reasons.push("Performance impact".to_string()),
            _ => {}
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

/// Format importance level for display
fn format_importance(importance: Importance) -> &'static str {
    match importance {
        Importance::Critical => "Critical",
        Importance::High => "High",
        Importance::Medium => "Medium",
        Importance::Low => "Low",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, CommentCount, Issue, IssueState};
    use crate::intelligence::{MatchedRule, PriorityScore};
    use jiff::Timestamp;

    #[test]
    fn test_build_context_prompt() {
        let labels = vec![Label {
            name: "backend".to_string(),
            description: "Backend services".to_string(),
            watch_rules: vec!["api_changes".to_string()],
            importance: Importance::High,
            context: "Focus on API stability and performance".to_string(),
        }];

        let mut repo_importances = HashMap::new();
        repo_importances.insert("test/repo".to_string(), Importance::Critical);

        let prompt = build_context_prompt(&labels, &repo_importances);

        assert!(prompt.contains("backend"));
        assert!(prompt.contains("API stability"));
        assert!(prompt.contains("Critical: test/repo"));
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
            labels: vec![],
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
            matched_rules: vec![MatchedRule {
                rule_type: "security_issues".to_string(),
                matched_text: "security".to_string(),
                confidence: 1.0,
            }],
            importance: Importance::High,
            context: None,
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
            matched_rules: vec![],
            importance: Importance::Medium,
            context: None,
        };

        let urgency = determine_urgency(&prioritized);
        assert_eq!(urgency, Urgency::High);
    }
}
