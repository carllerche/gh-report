use crate::config::{Config, Importance};
use crate::github::{Issue, RepoActivity};
use std::collections::BTreeMap;

mod context;
mod scoring;
pub use context::{build_context_prompt, extract_action_items};
pub use scoring::{calculate_priority_score, PriorityScore};

/// Intelligent filtering and analysis of GitHub activities
pub struct IntelligentAnalyzer<'a> {
    _config: &'a Config, // Keep for future use
}

impl<'a> IntelligentAnalyzer<'a> {
    pub fn new(config: &'a Config) -> Self {
        IntelligentAnalyzer { _config: config }
    }

    /// Analyze activities and return prioritized, filtered results
    pub fn analyze(&self, activities: &BTreeMap<String, RepoActivity>) -> AnalysisResult {
        let mut prioritized_issues = Vec::new();

        // Process each repository's activities
        for (repo_name, activity) in activities {
            // Determine repository importance (default to Medium for now)
            let importance = Importance::Medium;

            // Process all issues and PRs
            let mut all_items = Vec::new();
            all_items.extend(activity.new_issues.iter());
            all_items.extend(activity.updated_issues.iter());
            all_items.extend(activity.new_prs.iter());
            all_items.extend(activity.updated_prs.iter());

            for issue in all_items {
                // Calculate priority score based on basic metrics
                let score = calculate_priority_score(issue, importance, issue.is_pull_request);

                prioritized_issues.push(PrioritizedIssue {
                    issue: issue.clone(),
                    repo: repo_name.clone(),
                    score,
                    importance,
                });
            }
        }

        // Sort by priority score (highest first)
        prioritized_issues.sort_by(|a, b| b.score.total.cmp(&a.score.total));

        // Build simple context for AI summarization
        let context_prompt = build_context_prompt();

        // Extract potential action items
        let action_items = extract_action_items(&prioritized_issues);

        AnalysisResult {
            prioritized_issues,
            context_prompt,
            action_items,
        }
    }
}

/// Result of intelligent analysis
#[derive(Debug)]
pub struct AnalysisResult {
    pub prioritized_issues: Vec<PrioritizedIssue>,
    pub context_prompt: String,
    pub action_items: Vec<ActionItem>,
}

/// An issue with priority scoring and context
#[derive(Debug, Clone)]
pub struct PrioritizedIssue {
    pub issue: Issue,
    pub repo: String,
    pub score: PriorityScore,
    pub importance: Importance,
}

/// A suggested action item
#[derive(Debug, Clone)]
pub struct ActionItem {
    pub description: String,
    pub issue: Issue,
    pub repo: String,
    pub urgency: Urgency,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, CommentCount, IssueState, Label as GHLabel};
    use jiff::Timestamp;

    #[test]
    fn test_intelligent_analyzer_creation() {
        let config = Config::default();
        let analyzer = IntelligentAnalyzer::new(&config);

        let activities = BTreeMap::new();
        let result = analyzer.analyze(&activities);

        assert!(result.prioritized_issues.is_empty());
        assert!(result.action_items.is_empty());
    }

    #[test]
    fn test_analyze_with_issues() {
        let config = Config::default();
        let analyzer = IntelligentAnalyzer::new(&config);

        let mut activities = BTreeMap::new();
        let mut repo_activity = RepoActivity::default();

        repo_activity.new_issues.push(Issue {
            number: 42,
            title: "Security vulnerability found".to_string(),
            body: Some("Critical security issue".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "security-bot".to_string(),
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
            comments: CommentCount { total_count: 5 },
            is_pull_request: false,
        });

        activities.insert("test/repo".to_string(), repo_activity);

        let result = analyzer.analyze(&activities);

        // Should match security_issues watch rule
        assert!(!result.prioritized_issues.is_empty());
    }
}
