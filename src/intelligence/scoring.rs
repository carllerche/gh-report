use crate::config::Importance;
use crate::github::Issue;
use crate::intelligence::MatchedRule;
use jiff::Timestamp;

/// Priority score for an issue or PR
#[derive(Debug, Clone)]
pub struct PriorityScore {
    pub total: u32,
    pub importance_score: u32,
    pub recency_score: u32,
    pub activity_score: u32,
    pub rule_match_score: u32,
    pub label_score: u32,
}

/// Calculate priority score for an issue
pub fn calculate_priority_score(
    issue: &Issue,
    repo_importance: Importance,
    matched_rules: &[MatchedRule],
    is_pr: bool,
) -> PriorityScore {
    let mut score = PriorityScore {
        total: 0,
        importance_score: 0,
        recency_score: 0,
        activity_score: 0,
        rule_match_score: 0,
        label_score: 0,
    };
    
    // 1. Repository importance (0-40 points)
    score.importance_score = match repo_importance {
        Importance::Critical => 40,
        Importance::High => 30,
        Importance::Medium => 20,
        Importance::Low => 10,
    };
    
    // 2. Recency score (0-30 points)
    let now = Timestamp::now();
    let age_hours = ((now - issue.updated_at).get_hours() as u32).max(1);
    score.recency_score = match age_hours {
        0..=6 => 30,      // Last 6 hours
        7..=24 => 25,     // Last day
        25..=72 => 20,    // Last 3 days
        73..=168 => 15,   // Last week
        169..=336 => 10,  // Last 2 weeks
        _ => 5,           // Older
    };
    
    // 3. Activity score (0-20 points)
    score.activity_score = (issue.comments.total_count.min(10) * 2) as u32;
    
    // 4. Rule match score (0-30 points)
    for matched_rule in matched_rules {
        let rule_points = match matched_rule.rule_type.as_str() {
            "security_issues" => 30,
            "breaking_changes" => 25,
            "api_changes" => 20,
            "review_requests" => 15,
            "performance" => 15,
            "mentions" => 10,
            _ => 5,
        };
        score.rule_match_score = score.rule_match_score.max(rule_points);
    }
    
    // 5. Label score (0-20 points)
    for label in &issue.labels {
        let label_name = label.name.to_lowercase();
        let label_points = if label_name.contains("security") || label_name.contains("critical") {
            20
        } else if label_name.contains("bug") || label_name.contains("urgent") {
            15
        } else if label_name.contains("feature") || label_name.contains("enhancement") {
            10
        } else if label_name.contains("documentation") || label_name.contains("test") {
            5
        } else {
            2
        };
        score.label_score = score.label_score.max(label_points);
    }
    
    // 6. PR bonus (additional 10 points for PRs)
    if is_pr {
        score.total += 10;
    }
    
    // Calculate total
    score.total += score.importance_score 
        + score.recency_score 
        + score.activity_score 
        + score.rule_match_score 
        + score.label_score;
    
    score
}

/// Categorize score into priority level
pub fn score_to_priority(score: u32) -> Priority {
    match score {
        0..=30 => Priority::Low,
        31..=60 => Priority::Medium,
        61..=90 => Priority::High,
        _ => Priority::Critical,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, IssueState, CommentCount, Label};
    use jiff::{Timestamp, ToSpan};
    
    #[test]
    fn test_priority_scoring() {
        let now = Timestamp::now();
        let issue = Issue {
            number: 42,
            title: "Test Issue".to_string(),
            body: Some("Test body".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "testuser".to_string(),
                user_type: None,
            },
            created_at: now - 24_i64.hours(),
            updated_at: now - 2_i64.hours(),
            labels: vec![Label {
                name: "bug".to_string(),
                color: Some("red".to_string()),
                description: None,
            }],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 3 },
            is_pull_request: false,
        };
        
        let matched_rules = vec![MatchedRule {
            rule_type: "security_issues".to_string(),
            matched_text: "security".to_string(),
            confidence: 1.0,
        }];
        
        let score = calculate_priority_score(
            &issue,
            Importance::High,
            &matched_rules,
            false,
        );
        
        assert_eq!(score.importance_score, 30); // High importance
        assert_eq!(score.recency_score, 30);     // Last 6 hours
        assert_eq!(score.activity_score, 6);      // 3 comments * 2
        assert_eq!(score.rule_match_score, 30);  // Security rule
        assert_eq!(score.label_score, 15);       // Bug label
        
        let priority = score_to_priority(score.total);
        assert_eq!(priority, Priority::Critical);
    }
    
    #[test]
    fn test_pr_bonus() {
        let now = Timestamp::now();
        let pr = Issue {
            number: 100,
            title: "Feature PR".to_string(),
            body: None,
            state: IssueState::Open,
            author: Author {
                login: "dev".to_string(),
                user_type: None,
            },
            created_at: now,
            updated_at: now,
            labels: vec![],
            url: "https://github.com/test/repo/pull/100".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: true,
        };
        
        let score = calculate_priority_score(
            &pr,
            Importance::Medium,
            &[],
            true,
        );
        
        // Should have PR bonus
        assert!(score.total >= 10);
    }
}