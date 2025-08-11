use crate::config::{Config, Label, Importance};
use crate::github::{Issue, RepoActivity};
use std::collections::{BTreeMap, HashMap};

mod scoring;
mod watch_rules;
mod context;

pub use scoring::{PriorityScore, calculate_priority_score};
pub use watch_rules::{WatchRuleEngine, MatchedRule};
pub use context::{build_context_prompt, extract_action_items};

/// Intelligent filtering and analysis of GitHub activities
pub struct IntelligentAnalyzer<'a> {
    config: &'a Config,
    engine: WatchRuleEngine,
}

impl<'a> IntelligentAnalyzer<'a> {
    pub fn new(config: &'a Config) -> Self {
        let engine = WatchRuleEngine::new(&config.watch_rules);
        IntelligentAnalyzer { config, engine }
    }
    
    /// Analyze activities and return prioritized, filtered results
    pub fn analyze(
        &self,
        activities: &BTreeMap<String, RepoActivity>,
    ) -> AnalysisResult {
        let mut prioritized_issues = Vec::new();
        let mut matched_rules: HashMap<String, Vec<MatchedRule>> = HashMap::new();
        let mut repo_importances: HashMap<String, Importance> = HashMap::new();
        
        // Process each repository's activities
        for (repo_name, activity) in activities {
            // Determine repository importance
            let importance = self.get_repo_importance(repo_name);
            repo_importances.insert(repo_name.clone(), importance);
            
            // Get applicable labels and context for this repo
            let (labels, context) = self.get_repo_labels_and_context(repo_name);
            
            // Process all issues and PRs
            let mut all_items = Vec::new();
            all_items.extend(activity.new_issues.iter());
            all_items.extend(activity.updated_issues.iter());
            all_items.extend(activity.new_prs.iter());
            all_items.extend(activity.updated_prs.iter());
            
            for issue in all_items {
                // Check watch rules
                let matches = self.engine.check_issue(issue, &labels);
                
                if !matches.is_empty() {
                    // Calculate priority score
                    let score = calculate_priority_score(
                        issue,
                        importance,
                        &matches,
                        issue.is_pull_request,
                    );
                    
                    prioritized_issues.push(PrioritizedIssue {
                        issue: issue.clone(),
                        repo: repo_name.clone(),
                        score,
                        matched_rules: matches.clone(),
                        importance,
                        context: context.clone(),
                    });
                    
                    // Track matched rules
                    matched_rules
                        .entry(repo_name.clone())
                        .or_insert_with(Vec::new)
                        .extend(matches);
                }
            }
        }
        
        // Sort by priority score (highest first)
        prioritized_issues.sort_by(|a, b| b.score.total.cmp(&a.score.total));
        
        // Build context for AI summarization
        let context_prompt = build_context_prompt(&self.config.labels, &repo_importances);
        
        // Extract potential action items
        let action_items = extract_action_items(&prioritized_issues);
        
        AnalysisResult {
            prioritized_issues,
            matched_rules,
            context_prompt,
            action_items,
            repo_importances,
        }
    }
    
    /// Get the importance level for a repository
    fn get_repo_importance(&self, repo_name: &str) -> Importance {
        // Check if repo has an importance override
        for repo_config in &self.config.repos {
            if repo_config.name == repo_name {
                if let Some(importance) = repo_config.importance_override {
                    return importance;
                }
            }
        }
        
        // Default to medium
        Importance::Medium
    }
    
    /// Get labels and context for a repository
    fn get_repo_labels_and_context(&self, repo_name: &str) -> (Vec<String>, Option<String>) {
        for repo_config in &self.config.repos {
            if repo_config.name == repo_name {
                let mut labels = repo_config.labels.clone();
                
                // Add any watch rules specific to this repo
                if let Some(rules) = &repo_config.watch_rules {
                    labels.extend(rules.clone());
                }
                
                return (labels, repo_config.custom_context.clone());
            }
        }
        
        // No specific config for this repo
        (vec![], None)
    }
}

/// Result of intelligent analysis
#[derive(Debug)]
pub struct AnalysisResult {
    pub prioritized_issues: Vec<PrioritizedIssue>,
    pub matched_rules: HashMap<String, Vec<MatchedRule>>,
    pub context_prompt: String,
    pub action_items: Vec<ActionItem>,
    pub repo_importances: HashMap<String, Importance>,
}

/// An issue with priority scoring and context
#[derive(Debug, Clone)]
pub struct PrioritizedIssue {
    pub issue: Issue,
    pub repo: String,
    pub score: PriorityScore,
    pub matched_rules: Vec<MatchedRule>,
    pub importance: Importance,
    pub context: Option<String>,
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
    use crate::github::{Author, IssueState, CommentCount, Label as GHLabel};
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