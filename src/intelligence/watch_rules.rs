use crate::github::Issue;
use std::collections::HashMap;

/// Engine for matching issues against watch rules
pub struct WatchRuleEngine {
    rules: HashMap<String, Vec<String>>,
}

impl WatchRuleEngine {
    pub fn new(rules: &HashMap<String, Vec<String>>) -> Self {
        WatchRuleEngine {
            rules: rules.clone(),
        }
    }
    
    /// Check if an issue matches any watch rules
    pub fn check_issue(&self, issue: &Issue, active_labels: &[String]) -> Vec<MatchedRule> {
        let mut matches = Vec::new();
        
        // Build searchable text from issue
        let searchable_text = self.build_searchable_text(issue);
        let searchable_lower = searchable_text.to_lowercase();
        
        // Check each active label's watch rules
        for label in active_labels {
            if let Some(patterns) = self.rules.get(label) {
                // Special case: empty patterns means match everything
                if patterns.is_empty() && label == "all_activity" {
                    matches.push(MatchedRule {
                        rule_type: label.clone(),
                        matched_text: "all".to_string(),
                        confidence: 1.0,
                    });
                    continue;
                }
                
                // Check each pattern
                for pattern in patterns {
                    let pattern_lower = pattern.to_lowercase();
                    
                    // Handle special patterns
                    if pattern.starts_with("@{") && pattern.ends_with("}") {
                        // Username mention pattern - skip for now
                        continue;
                    }
                    
                    // Simple substring matching (could be enhanced with regex)
                    if searchable_lower.contains(&pattern_lower) {
                        matches.push(MatchedRule {
                            rule_type: label.clone(),
                            matched_text: pattern.clone(),
                            confidence: 1.0,
                        });
                        break; // Only need one match per rule type
                    }
                }
            }
        }
        
        // Check for label-based matches
        for gh_label in &issue.labels {
            let label_lower = gh_label.name.to_lowercase();
            
            // Check security labels
            if label_lower.contains("security") || label_lower.contains("vulnerability") {
                if !matches.iter().any(|m| m.rule_type == "security_issues") {
                    matches.push(MatchedRule {
                        rule_type: "security_issues".to_string(),
                        matched_text: gh_label.name.clone(),
                        confidence: 0.9,
                    });
                }
            }
            
            // Check breaking change labels
            if label_lower.contains("breaking") || label_lower.contains("major") {
                if !matches.iter().any(|m| m.rule_type == "breaking_changes") {
                    matches.push(MatchedRule {
                        rule_type: "breaking_changes".to_string(),
                        matched_text: gh_label.name.clone(),
                        confidence: 0.9,
                    });
                }
            }
        }
        
        matches
    }
    
    /// Build searchable text from an issue
    fn build_searchable_text(&self, issue: &Issue) -> String {
        let mut text = String::new();
        
        // Add title
        text.push_str(&issue.title);
        text.push(' ');
        
        // Add body if present
        if let Some(body) = &issue.body {
            text.push_str(body);
            text.push(' ');
        }
        
        // Add labels
        for label in &issue.labels {
            text.push_str(&label.name);
            text.push(' ');
        }
        
        text
    }
    
    /// Check if a repository matches any watch rules
    pub fn check_repo_patterns(&self, repo_name: &str, patterns: &[String]) -> Vec<MatchedRule> {
        let mut matches = Vec::new();
        let repo_lower = repo_name.to_lowercase();
        
        for pattern in patterns {
            // Check each pattern against repo name
            for (rule_type, rule_patterns) in &self.rules {
                if rule_patterns.contains(pattern) {
                    matches.push(MatchedRule {
                        rule_type: rule_type.clone(),
                        matched_text: pattern.clone(),
                        confidence: 0.8,
                    });
                }
            }
            
            // Direct pattern matching
            if repo_lower.contains(&pattern.to_lowercase()) {
                matches.push(MatchedRule {
                    rule_type: "repo_pattern".to_string(),
                    matched_text: pattern.clone(),
                    confidence: 0.7,
                });
            }
        }
        
        matches
    }
}

/// A matched watch rule
#[derive(Debug, Clone)]
pub struct MatchedRule {
    pub rule_type: String,
    pub matched_text: String,
    pub confidence: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, IssueState, CommentCount, Label};
    use jiff::Timestamp;
    
    #[test]
    fn test_watch_rule_matching() {
        let mut rules = HashMap::new();
        rules.insert(
            "security_issues".to_string(),
            vec!["security".to_string(), "vulnerability".to_string()],
        );
        rules.insert(
            "breaking_changes".to_string(),
            vec!["BREAKING".to_string(), "migration".to_string()],
        );
        
        let engine = WatchRuleEngine::new(&rules);
        
        let issue = Issue {
            number: 42,
            title: "Security vulnerability in auth module".to_string(),
            body: Some("Found a critical security issue".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "security-bot".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        };
        
        let matches = engine.check_issue(&issue, &["security_issues".to_string()]);
        
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_type, "security_issues");
        assert_eq!(matches[0].matched_text, "security");
    }
    
    #[test]
    fn test_label_based_matching() {
        let rules = HashMap::new();
        let engine = WatchRuleEngine::new(&rules);
        
        let issue = Issue {
            number: 100,
            title: "Update API".to_string(),
            body: None,
            state: IssueState::Open,
            author: Author {
                login: "dev".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![
                Label {
                    name: "breaking-change".to_string(),
                    color: Some("red".to_string()),
                    description: None,
                },
            ],
            url: "https://github.com/test/repo/issues/100".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        };
        
        let matches = engine.check_issue(&issue, &[]);
        
        // Should match based on label
        assert!(!matches.is_empty());
        assert_eq!(matches[0].rule_type, "breaking_changes");
        assert_eq!(matches[0].confidence, 0.9);
    }
    
    #[test]
    fn test_all_activity_rule() {
        let mut rules = HashMap::new();
        rules.insert("all_activity".to_string(), vec![]);
        
        let engine = WatchRuleEngine::new(&rules);
        
        let issue = Issue {
            number: 1,
            title: "Random issue".to_string(),
            body: None,
            state: IssueState::Open,
            author: Author {
                login: "user".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![],
            url: "https://github.com/test/repo/issues/1".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        };
        
        let matches = engine.check_issue(&issue, &["all_activity".to_string()]);
        
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_type, "all_activity");
    }
}