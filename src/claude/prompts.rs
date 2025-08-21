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
            prompt.push_str(&format!(
                "### New Pull Requests ({})\n",
                activity.new_prs.len()
            ));
            for pr in &activity.new_prs {
                let state_str = match pr.state {
                    crate::github::IssueState::Open => "Open",
                    crate::github::IssueState::Closed => "Closed",
                    crate::github::IssueState::Merged => "Merged",
                };
                prompt.push_str(&format!(
                    "- [PR #{}]({}): {} (State: {}, by [@{}](https://github.com/{}))\n",
                    pr.number, pr.url, pr.title, state_str, pr.author.login, pr.author.login
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
            prompt.push_str(&format!(
                "### Updated Pull Requests ({})\n",
                activity.updated_prs.len()
            ));
            for pr in &activity.updated_prs {
                let state_str = match pr.state {
                    crate::github::IssueState::Open => "Open",
                    crate::github::IssueState::Closed => "Closed",
                    crate::github::IssueState::Merged => "Merged",
                };
                prompt.push_str(&format!(
                    "- [PR #{}]({}): {} (State: {}, comments: {})\n",
                    pr.number, pr.url, pr.title, state_str, pr.comments.total_count
                ));
            }
            prompt.push('\n');
        }

        if !activity.new_issues.is_empty() {
            prompt.push_str(&format!("### New Issues ({})\n", activity.new_issues.len()));
            for issue in &activity.new_issues {
                let state_str = match issue.state {
                    crate::github::IssueState::Open => "Open",
                    crate::github::IssueState::Closed => "Closed",
                    crate::github::IssueState::Merged => "Merged",
                };
                prompt.push_str(&format!(
                    "- [Issue #{}]({}): {} (State: {}, by [@{}](https://github.com/{}))\n",
                    issue.number, issue.url, issue.title, state_str, issue.author.login, issue.author.login
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
            prompt.push_str(&format!(
                "### Updated Issues ({})\n",
                activity.updated_issues.len()
            ));
            for issue in &activity.updated_issues {
                let state_str = match issue.state {
                    crate::github::IssueState::Open => "Open",
                    crate::github::IssueState::Closed => "Closed",
                    crate::github::IssueState::Merged => "Merged",
                };
                prompt.push_str(&format!(
                    "- [Issue #{}]({}): {} (State: {}, comments: {})\n",
                    issue.number, issue.url, issue.title, state_str, issue.comments.total_count
                ));
            }
            prompt.push('\n');
        }
    }

    prompt.push_str("\nProvide a summary that:\n");
    prompt.push_str("1. Highlights the most important items that need attention\n");
    prompt.push_str("2. Groups related activities together\n");
    prompt.push_str("3. Identifies any blocking issues or urgent matters\n");
    prompt.push_str("4. Suggests action items ONLY for Open issues/PRs that need attention\n");
    prompt.push_str("5. Celebrates completed work (Merged PRs, Closed issues) separately\n");
    prompt.push_str("6. Keep it concise - focus on what matters most\n");
    prompt.push_str("7. When mentioning specific issues or PRs, always include the URL in markdown link format: [#123](URL)\n");
    prompt.push_str("8. When mentioning users, make them clickable using the format: [@username](https://github.com/username)\n");
    prompt.push_str("\nIMPORTANT: Pay attention to the State field for each item:\n");
    prompt.push_str("- Open: Needs attention, suggest actions if appropriate\n");
    prompt.push_str("- Merged: Completed work, acknowledge the accomplishment\n");
    prompt.push_str("- Closed: Resolved, mention briefly but don't suggest actions\n");

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

    prompt.push_str(
        r#"Provide:
1. A brief summary of the issue/PR (2-3 sentences)
2. Key points or decisions made
3. Current status and next steps if clear
4. Any blockers or concerns raised"#,
    );

    prompt
}

/// Generate a prompt for filtering activities by importance
pub fn filter_activities_prompt(activities_summary: &str, watch_rules: &[String]) -> String {
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

/// Generate a maintainer-focused prompt for summarizing a specific issue/PR
pub fn summarize_issue_for_maintainer(
    issue_title: &str,
    issue_body: &str,
    issue_state: &str,
    issue_author: &str,
    issue_labels: &[String],
    issue_url: &str,
    comments: &[(String, String)], // (author, body) pairs
    include_recommendations: bool,
) -> String {
    let mut prompt = format!(
        r#"You are helping a project maintainer quickly understand and make decisions about a GitHub issue/PR. 

**Issue Details:**
- Title: {}
- State: {}
- Author: @{}
- Labels: {}
- URL: {}

**Description:**
{}

"#,
        issue_title,
        issue_state,
        issue_author,
        if issue_labels.is_empty() {
            "none".to_string()
        } else {
            issue_labels.join(", ")
        },
        issue_url,
        issue_body
    );

    if !comments.is_empty() {
        prompt.push_str("**Discussion:**\n");
        for (i, (author, body)) in comments.iter().enumerate() {
            prompt.push_str(&format!("Comment {} by @{}:\n{}\n\n", i + 1, author, body));
        }
    }

    if include_recommendations {
        prompt.push_str(r#"
**Provide the following analysis:**

## Required Action
Clearly state what action you, as the maintainer, need to take. Be specific and actionable.

## Recommendations  
Provide exactly 2 specific, practical recommendations for how to handle this issue/PR. Focus on concrete next steps.

## Current Status
- State (open/closed/merged)
- Key participants and their roles
- Decision points reached so far
- Any blockers or dependencies

## Summary
Concise overview of what this issue/PR is about and why it matters to the project.

## Key Discussion Points
- Important quotes or decisions from the discussion
- Different viewpoints presented
- Technical considerations raised
- Community concerns or feedback

## Recent Activity
What has happened most recently that the maintainer should know about.

**Format as markdown with clear headings. Focus on helping the maintainer make informed decisions quickly.**

**Important formatting notes:**
- When mentioning users, make them clickable: [@username](https://github.com/username)
- When referencing issues/PRs, include clickable links: [#123](URL)"#);
    } else {
        prompt.push_str(
            r#"
**Provide a factual summary including:**

## Current Status
- State (open/closed/merged) 
- Key participants and their roles
- Timeline of major events

## Summary
Objective overview of what this issue/PR addresses.

## Key Discussion Points
- Main technical points discussed
- Different approaches considered
- Concerns or blockers identified

## Recent Activity
Latest developments in chronological order.

**Format as markdown. Present facts objectively without recommendations.**

**Important formatting notes:**
- When mentioning users, make them clickable: [@username](https://github.com/username)
- When referencing issues/PRs, include clickable links: [#123](URL)"#,
        );
    }

    prompt
}

/// Generate a filename-safe version of an issue title
pub fn generate_issue_filename(repo_name: &str, issue_number: u32, title: &str) -> String {
    // Extract just the repo name (not owner/repo)
    let repo = repo_name.split('/').nth(1).unwrap_or(repo_name);

    // Sanitize title for filesystem
    let clean_title = title
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            ' ' => '-',
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('-')
        .trim_matches('_')
        .to_lowercase();

    // Truncate title to reasonable length
    let truncated_title = if clean_title.len() > 50 {
        format!("{}...", &clean_title[..47])
    } else {
        clean_title
    };

    format!("{}-{}-{}.md", repo, issue_number, truncated_title)
}

/// Generate a specialized prompt for Claude Code review of a PR
pub fn review_pr_for_maintainer(
    pr_title: &str,
    pr_body: &str,
    pr_state: &str,
    pr_author: &str,
    pr_labels: &[String],
    pr_url: &str,
    comments: &[(String, String)], // (author, body) pairs
    diff_summary: &str,            // Summary of file changes
    include_recommendations: bool,
) -> String {
    let mut prompt = format!(
        r#"You are providing a maintainer-focused code review for a GitHub pull request using Claude Code's capabilities.

**Pull Request Details:**
- Title: {}
- State: {}
- Author: @{}
- Labels: {}
- URL: {}

**Description:**
{}

**Code Changes Summary:**
{}

"#,
        pr_title,
        pr_state,
        pr_author,
        if pr_labels.is_empty() {
            "none".to_string()
        } else {
            pr_labels.join(", ")
        },
        pr_url,
        pr_body,
        diff_summary
    );

    if !comments.is_empty() {
        prompt.push_str("**Discussion:**\n");
        for (i, (author, body)) in comments.iter().enumerate() {
            prompt.push_str(&format!("Comment {} by @{}:\n{}\n\n", i + 1, author, body));
        }
    }

    if include_recommendations {
        prompt.push_str(r#"
**Provide a comprehensive code review analysis:**

## Code Review Summary
Provide a high-level assessment of the code changes, highlighting the main purpose and overall quality.

## Key Findings
- **Strengths**: What's well-implemented in this PR
- **Concerns**: Issues that need attention (bugs, performance, security, maintainability)
- **Architecture**: How well the changes fit with the existing codebase structure

## Required Actions
List specific, actionable items the maintainer should address before merging, prioritized by importance.

## Recommendations
Provide exactly 2 specific recommendations for improving this PR or the development process.

## Technical Assessment
- **Code Quality**: Style, clarity, and maintainability
- **Testing**: Coverage and test quality assessment
- **Performance**: Potential performance implications
- **Security**: Security considerations and potential vulnerabilities
- **Breaking Changes**: Impact on existing APIs or behavior

## Discussion Analysis
Key technical points from the PR discussion and how they've been addressed.

## Merge Decision Support
Clear guidance on whether this PR is ready to merge, needs revisions, or requires further discussion.

**Format as markdown with clear headings. Focus on helping the maintainer make informed decisions about the code quality and merge readiness.**

**Important formatting notes:**
- When mentioning users, make them clickable: [@username](https://github.com/username)
- When referencing issues/PRs, include clickable links: [#123](URL)"#);
    } else {
        prompt.push_str(
            r#"
**Provide an objective code review analysis:**

## Code Changes Overview
Factual summary of what the code changes accomplish.

## Technical Assessment
- **Files Modified**: Key files and types of changes
- **Code Quality**: Objective assessment of style and structure
- **Testing**: Test coverage and types of tests included
- **Complexity**: Overall complexity of the changes

## Discussion Summary
Key technical points discussed in the PR comments.

## Current Status
- State (open/closed/merged)
- Review status and approvals
- CI/CD status if mentioned

**Format as markdown. Present technical facts objectively without recommendations.**

**Important formatting notes:**
- When mentioning users, make them clickable: [@username](https://github.com/username)
- When referencing issues/PRs, include clickable links: [#123](URL)"#,
        );
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, CommentCount, Issue, IssueState};
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
        assert!(prompt.contains("[Issue #42]"));
    }

    #[test]
    fn test_generate_title_prompt() {
        let summary = "Fixed critical bugs and added new features";
        let prompt = generate_title_prompt(summary);

        assert!(prompt.contains(summary));
        assert!(prompt.contains("8 words or fewer"));
    }

    #[test]
    fn test_summarize_issue_for_maintainer_with_recommendations() {
        let prompt = summarize_issue_for_maintainer(
            "Memory leak in async runtime",
            "Detailed description of the memory leak...",
            "open",
            "user123",
            &vec!["bug".to_string(), "critical".to_string()],
            "https://github.com/owner/repo/issues/123",
            &vec![
                (
                    "reviewer1".to_string(),
                    "I can reproduce this issue".to_string(),
                ),
                (
                    "maintainer".to_string(),
                    "Let's prioritize this fix".to_string(),
                ),
            ],
            true,
        );

        assert!(prompt.contains("Memory leak in async runtime"));
        assert!(prompt.contains("@user123"));
        assert!(prompt.contains("bug, critical"));
        assert!(prompt.contains("Required Action"));
        assert!(prompt.contains("Recommendations"));
        assert!(prompt.contains("@reviewer1"));
        assert!(prompt.contains("I can reproduce this issue"));
    }

    #[test]
    fn test_summarize_issue_for_maintainer_without_recommendations() {
        let prompt = summarize_issue_for_maintainer(
            "Feature request: Add new API",
            "Description of the feature...",
            "open",
            "contributor",
            &vec![],
            "https://github.com/owner/repo/issues/456",
            &vec![],
            false,
        );

        assert!(prompt.contains("Feature request: Add new API"));
        assert!(prompt.contains("@contributor"));
        assert!(prompt.contains("Labels: none"));
        assert!(prompt.contains("Current Status"));
        assert!(!prompt.contains("Required Action"));
        assert!(!prompt.contains("Recommendations"));
    }

    #[test]
    fn test_generate_issue_filename() {
        // Test basic functionality
        let filename = generate_issue_filename("tokio-rs/tokio", 123, "Fix memory leak in runtime");
        assert_eq!(filename, "tokio-123-fix-memory-leak-in-runtime.md");

        // Test with special characters
        let filename =
            generate_issue_filename("rust-lang/rust", 456, "Add support for async/await syntax");
        assert_eq!(filename, "rust-456-add-support-for-async_await-syntax.md");

        // Test with long title
        let long_title =
            "This is a very long issue title that should be truncated to avoid filesystem issues";
        let filename = generate_issue_filename("microsoft/TypeScript", 789, long_title);
        assert_eq!(
            filename,
            "TypeScript-789-this-is-a-very-long-issue-title-that-should-be-....md"
        );

        // Test edge cases
        let filename =
            generate_issue_filename("user/repo", 1, "Fix!@#$%^&*()+={}[]|\\:;\"'<>?/.,`~");
        assert_eq!(filename, "repo-1-fix.md");
    }

    #[test]
    fn test_review_pr_for_maintainer() {
        let prompt = review_pr_for_maintainer(
            "Add async/await support to core library",
            "This PR introduces async/await syntax support with full backwards compatibility.",
            "open",
            "contributor123",
            &vec!["enhancement".to_string(), "breaking-change".to_string()],
            "https://github.com/owner/repo/pull/456",
            &vec![
                ("reviewer1".to_string(), "The implementation looks solid".to_string()),
                ("maintainer".to_string(), "Let's ensure all tests pass".to_string()),
            ],
            "Modified 15 files: 8 Rust files, 4 test files, 3 documentation files. Added 342 lines, removed 89 lines.",
            true,
        );

        assert!(prompt.contains("Add async/await support to core library"));
        assert!(prompt.contains("@contributor123"));
        assert!(prompt.contains("enhancement, breaking-change"));
        assert!(prompt.contains("Code Review Summary"));
        assert!(prompt.contains("Required Actions"));
        assert!(prompt.contains("Technical Assessment"));
        assert!(prompt.contains("Modified 15 files"));
        assert!(prompt.contains("@reviewer1"));
        assert!(prompt.contains("The implementation looks solid"));
    }

    #[test]
    fn test_review_pr_for_maintainer_without_recommendations() {
        let prompt = review_pr_for_maintainer(
            "Fix typo in documentation",
            "Simple typo fix in README.md",
            "merged",
            "docs-contributor",
            &vec![],
            "https://github.com/owner/repo/pull/789",
            &vec![],
            "Modified 1 file: README.md. Added 1 line, removed 1 line.",
            false,
        );

        assert!(prompt.contains("Fix typo in documentation"));
        assert!(prompt.contains("@docs-contributor"));
        assert!(prompt.contains("Labels: none"));
        assert!(prompt.contains("Code Changes Overview"));
        assert!(prompt.contains("Technical Assessment"));
        assert!(!prompt.contains("Required Actions"));
        assert!(!prompt.contains("Recommendations"));
    }
}
