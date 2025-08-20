use crate::github::RepoActivity;
use std::collections::BTreeMap;

/// Generate a system prompt for GitHub activity summarization
pub fn system_prompt() -> String {
    r#"You are an AI assistant specialized in summarizing GitHub activity for software developers.
Your role is to analyze issues, pull requests, and comments to provide concise, actionable summaries.

Focus on:
1. Highlighting important decisions that need to be made
2. Identifying blocking issues or urgent matters
3. Summarizing key discussions and their outcomes
4. Grouping related activities together
5. Providing clear action items when relevant

Be concise but comprehensive. Use bullet points for clarity.
Prioritize information based on urgency and importance."#.to_string()
}

/// Generate a prompt for summarizing repository activities
pub fn summarize_activities_prompt(
    activities: &BTreeMap<String, RepoActivity>,
    context: Option<&str>,
) -> String {
    let mut prompt = String::new();
    
    if let Some(ctx) = context {
        prompt.push_str("User Context:\n");
        prompt.push_str(ctx);
        prompt.push_str("\n\n");
    }
    
    prompt.push_str("Please summarize the following GitHub activity:\n\n");
    
    for (repo_name, activity) in activities {
        prompt.push_str(&format!("## Repository: {}\n\n", repo_name));
        
        if !activity.new_prs.is_empty() {
            prompt.push_str(&format!("### New Pull Requests ({})\n", activity.new_prs.len()));
            for pr in &activity.new_prs {
                prompt.push_str(&format!(
                    "- PR #{}: {} (by @{}) - {}\n",
                    pr.number, pr.title, pr.author.login, pr.url
                ));
                if let Some(body) = &pr.body {
                    if !body.is_empty() && body.len() < 200 {
                        prompt.push_str(&format!("  {}\n", body.replace('\n', " ")));
                    }
                }
            }
            prompt.push('\n');
        }
        
        if !activity.updated_prs.is_empty() {
            prompt.push_str(&format!("### Updated Pull Requests ({})\n", activity.updated_prs.len()));
            for pr in &activity.updated_prs {
                prompt.push_str(&format!(
                    "- PR #{}: {} (comments: {}) - {}\n",
                    pr.number, pr.title, pr.comments.total_count, pr.url
                ));
            }
            prompt.push('\n');
        }
        
        if !activity.new_issues.is_empty() {
            prompt.push_str(&format!("### New Issues ({})\n", activity.new_issues.len()));
            for issue in &activity.new_issues {
                prompt.push_str(&format!(
                    "- Issue #{}: {} (by @{}) - {}\n",
                    issue.number, issue.title, issue.author.login, issue.url
                ));
                // Add labels if present
                if !issue.labels.is_empty() {
                    let labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
                    prompt.push_str(&format!("  Labels: {}\n", labels.join(", ")));
                }
            }
            prompt.push('\n');
        }
        
        if !activity.updated_issues.is_empty() {
            prompt.push_str(&format!("### Updated Issues ({})\n", activity.updated_issues.len()));
            for issue in &activity.updated_issues {
                prompt.push_str(&format!(
                    "- Issue #{}: {} (comments: {}) - {}\n",
                    issue.number, issue.title, issue.comments.total_count, issue.url
                ));
            }
            prompt.push('\n');
        }
    }
    
    prompt.push_str("\nProvide a summary that:\n");
    prompt.push_str("1. Highlights the most important items that need attention\n");
    prompt.push_str("2. Groups related activities together\n");
    prompt.push_str("3. Identifies any blocking issues or urgent matters\n");
    prompt.push_str("4. Suggests action items if applicable\n");
    prompt.push_str("5. Keep it concise - focus on what matters most\n");
    prompt.push_str("6. When mentioning specific issues or PRs, always include the URL in markdown link format: [#123](URL)\n");
    
    prompt
}

/// Generate a prompt for creating a short title
pub fn generate_title_prompt(summary: &str) -> String {
    format!(
        r#"Based on this GitHub activity summary, generate a short title (8 words or fewer) that captures the main theme or most important aspect:

{}

Provide only the title, no additional text or punctuation."#,
        summary
    )
}

/// Generate a prompt for summarizing issue/PR context
pub fn summarize_context_prompt(
    issue_title: &str,
    issue_body: &str,
    comments: &[String],
) -> String {
    let mut prompt = format!(
        r#"Summarize this GitHub issue/PR and its discussion:

Title: {}

Description:
{}

"#,
        issue_title, issue_body
    );
    
    if !comments.is_empty() {
        prompt.push_str("Recent Comments:\n");
        for (i, comment) in comments.iter().enumerate() {
            prompt.push_str(&format!("Comment {}:\n{}\n\n", i + 1, comment));
        }
    }
    
    prompt.push_str(r#"Provide:
1. A brief summary of the issue/PR (2-3 sentences)
2. Key points or decisions made
3. Current status and next steps if clear
4. Any blockers or concerns raised"#);
    
    prompt
}

/// Generate a prompt for filtering activities by importance
pub fn filter_activities_prompt(
    activities_summary: &str,
    watch_rules: &[String],
) -> String {
    let mut prompt = format!(
        r#"Given these watch rules for what's important:

Watch Rules:
"#
    );
    
    for rule in watch_rules {
        prompt.push_str(&format!("- {}\n", rule));
    }
    
    prompt.push_str(&format!(
        r#"

And this GitHub activity:

{}

Identify which items match the watch rules and explain why they're important.
Group results by priority: High, Medium, Low.
For each item, briefly explain which rule it matches and why it matters."#,
        activities_summary
    ));
    
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Issue, Author, IssueState, CommentCount};
    use jiff::Timestamp;
    
    #[test]
    fn test_system_prompt() {
        let prompt = system_prompt();
        assert!(prompt.contains("GitHub activity"));
        assert!(prompt.contains("concise"));
    }
    
    #[test]
    fn test_summarize_activities_prompt() {
        let mut activities = BTreeMap::new();
        let mut repo_activity = RepoActivity::default();
        
        repo_activity.new_issues.push(Issue {
            number: 42,
            title: "Test Issue".to_string(),
            body: Some("Issue body".to_string()),
            state: IssueState::Open,
            author: Author {
                login: "testuser".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        });
        
        activities.insert("test/repo".to_string(), repo_activity);
        
        let prompt = summarize_activities_prompt(&activities, Some("Focus on bug fixes"));
        
        assert!(prompt.contains("User Context:"));
        assert!(prompt.contains("Focus on bug fixes"));
        assert!(prompt.contains("Repository: test/repo"));
        assert!(prompt.contains("Issue #42: Test Issue"));
    }
    
    #[test]
    fn test_generate_title_prompt() {
        let summary = "Fixed critical bugs and added new features";
        let prompt = generate_title_prompt(summary);
        
        assert!(prompt.contains(summary));
        assert!(prompt.contains("8 words or fewer"));
    }
}