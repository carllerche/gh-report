use anyhow::Result;
use jiff::Timestamp;
use std::collections::BTreeMap;
use std::fmt::Write;

use crate::config::Config;
use crate::github::{Issue, IssueState, RepoActivity};
use crate::intelligence::AnalysisResult;

pub struct ReportTemplate<'a> {
    _config: &'a Config,
}

impl<'a> ReportTemplate<'a> {
    pub fn new(config: &'a Config) -> Self {
        ReportTemplate { _config: config }
    }

    pub fn render(
        &self,
        activities: &BTreeMap<String, RepoActivity>,
        since: Timestamp,
        now: Timestamp,
        errors: &[String],
    ) -> Result<String> {
        self.render_with_summary(activities, since, now, errors, None)
    }

    pub fn render_with_summary(
        &self,
        activities: &BTreeMap<String, RepoActivity>,
        since: Timestamp,
        now: Timestamp,
        errors: &[String],
        ai_summary: Option<&str>,
    ) -> Result<String> {
        self.render_with_intelligence(
            activities,
            since,
            now,
            errors,
            ai_summary,
            &AnalysisResult {
                prioritized_issues: vec![],
                matched_rules: std::collections::HashMap::new(),
                context_prompt: String::new(),
                action_items: vec![],
                repo_importances: std::collections::HashMap::new(),
            },
        )
    }

    pub fn render_with_intelligence(
        &self,
        activities: &BTreeMap<String, RepoActivity>,
        since: Timestamp,
        now: Timestamp,
        errors: &[String],
        ai_summary: Option<&str>,
        analysis: &AnalysisResult,
    ) -> Result<String> {
        let mut output = String::new();

        self.write_header(&mut output, since, now)?;

        if !errors.is_empty() {
            self.write_errors(&mut output, errors)?;
        }

        // Add action items if available
        if !analysis.action_items.is_empty() {
            writeln!(&mut output, "\n## Action Items\n")?;
            for (i, action) in analysis.action_items.iter().enumerate() {
                let urgency_text = match action.urgency {
                    crate::intelligence::Urgency::Critical => "[CRITICAL]",
                    crate::intelligence::Urgency::High => "[HIGH]",
                    crate::intelligence::Urgency::Medium => "[MEDIUM]",
                    crate::intelligence::Urgency::Low => "[LOW]",
                };
                writeln!(
                    &mut output,
                    "{}. {} {} - {} ([#{}]({}))",
                    i + 1,
                    urgency_text,
                    action.description,
                    action.reason,
                    action.issue.number,
                    action.issue.url
                )?;
            }
            writeln!(&mut output)?;
        }

        // Add highlights if available
        if let Some(summary) = ai_summary {
            writeln!(&mut output, "\n## Highlights\n")?;
            writeln!(&mut output, "{}", summary)?;
        }

        if activities.is_empty() {
            writeln!(&mut output, "\n## No Activity\n")?;
            writeln!(
                &mut output,
                "No issues or pull requests were updated in the specified time period."
            )?;
        } else {
            self.write_summary(&mut output, activities)?;

            // Add prioritized issues section if available
            if !analysis.prioritized_issues.is_empty() {
                writeln!(&mut output, "\n## Prioritized Items\n")?;

                // Show top 10 prioritized items
                for issue in analysis.prioritized_issues.iter().take(10) {
                    let type_str = if issue.issue.is_pull_request {
                        "PR"
                    } else {
                        "Issue"
                    };
                    writeln!(
                        &mut output,
                        "- **[{}]** {} [#{}]({}) - {} (Score: {})",
                        issue.repo,
                        type_str,
                        issue.issue.number,
                        issue.issue.url,
                        issue.issue.title,
                        issue.score.total
                    )?;
                }
                writeln!(&mut output)?;
            }

            self.write_activities(&mut output, activities)?;
        }

        self.write_footer(&mut output)?;

        Ok(output)
    }

    fn write_header(&self, output: &mut String, since: Timestamp, now: Timestamp) -> Result<()> {
        writeln!(output, "# GitHub Activity Report")?;
        writeln!(output)?;
        writeln!(
            output,
            "**Period**: {} to {}",
            since.strftime("%Y-%m-%d %H:%M"),
            now.strftime("%Y-%m-%d %H:%M")
        )?;
        writeln!(
            output,
            "**Generated**: {}",
            now.strftime("%Y-%m-%d %H:%M:%S")
        )?;
        Ok(())
    }

    fn write_errors(&self, output: &mut String, errors: &[String]) -> Result<()> {
        writeln!(output, "\n## Warnings\n")?;
        for error in errors {
            writeln!(output, "- {}", error)?;
        }
        Ok(())
    }

    fn write_summary(
        &self,
        output: &mut String,
        activities: &BTreeMap<String, RepoActivity>,
    ) -> Result<()> {
        writeln!(output, "\n## Summary\n")?;

        let mut total_new_issues = 0;
        let mut total_updated_issues = 0;
        let mut total_new_prs = 0;
        let mut total_updated_prs = 0;

        for activity in activities.values() {
            total_new_issues += activity.new_issues.len();
            total_updated_issues += activity.updated_issues.len();
            total_new_prs += activity.new_prs.len();
            total_updated_prs += activity.updated_prs.len();
        }

        writeln!(output, "- **Repositories**: {}", activities.len())?;
        writeln!(output, "- **New Issues**: {}", total_new_issues)?;
        writeln!(output, "- **Updated Issues**: {}", total_updated_issues)?;
        writeln!(output, "- **New Pull Requests**: {}", total_new_prs)?;
        writeln!(output, "- **Updated Pull Requests**: {}", total_updated_prs)?;

        Ok(())
    }

    fn write_activities(
        &self,
        output: &mut String,
        activities: &BTreeMap<String, RepoActivity>,
    ) -> Result<()> {
        writeln!(output, "\n## Activity by Repository\n")?;

        for (repo_name, activity) in activities {
            let total = activity.new_issues.len()
                + activity.updated_issues.len()
                + activity.new_prs.len()
                + activity.updated_prs.len();

            if total == 0 {
                continue;
            }

            writeln!(output, "### {}\n", repo_name)?;

            if !activity.new_prs.is_empty() {
                writeln!(output, "#### New Pull Requests\n")?;
                for pr in &activity.new_prs {
                    self.write_issue_line(output, pr)?;
                }
                writeln!(output)?;
            }

            if !activity.updated_prs.is_empty() {
                writeln!(output, "#### Updated Pull Requests\n")?;
                for pr in &activity.updated_prs {
                    self.write_issue_line(output, pr)?;
                }
                writeln!(output)?;
            }

            if !activity.new_issues.is_empty() {
                writeln!(output, "#### New Issues\n")?;
                for issue in &activity.new_issues {
                    self.write_issue_line(output, issue)?;
                }
                writeln!(output)?;
            }

            if !activity.updated_issues.is_empty() {
                writeln!(output, "#### Updated Issues\n")?;
                for issue in &activity.updated_issues {
                    self.write_issue_line(output, issue)?;
                }
                writeln!(output)?;
            }
        }

        Ok(())
    }

    fn write_issue_line(&self, output: &mut String, issue: &Issue) -> Result<()> {
        let state_text = match issue.state {
            IssueState::Open => "[OPEN]",
            IssueState::Closed => "[CLOSED]",
            IssueState::Merged => "[MERGED]",
        };

        let labels = if issue.labels.is_empty() {
            String::new()
        } else {
            let label_names: Vec<String> = issue
                .labels
                .iter()
                .map(|l| format!("`{}`", l.name))
                .collect();
            format!(" {}", label_names.join(" "))
        };

        writeln!(
            output,
            "- {} [#{}]({}) {}{} by [@{}](https://github.com/{})",
            state_text,
            issue.number,
            issue.url,
            issue.title,
            labels,
            issue.author.login,
            issue.author.login
        )?;

        Ok(())
    }

    fn write_footer(&self, output: &mut String) -> Result<()> {
        writeln!(output, "\n---")?;
        writeln!(
            output,
            "\n*Generated by gh-report v{}*",
            env!("CARGO_PKG_VERSION")
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Author, CommentCount, Issue, Label};
    use jiff::ToSpan;

    #[test]
    fn test_template_render_empty() {
        let config = Config::default();
        let template = ReportTemplate::new(&config);
        let activities = BTreeMap::new();
        let now = Timestamp::now();
        let since = now - 24_i64.hours();

        let result = template.render(&activities, since, now, &[]).unwrap();
        assert!(result.contains("No Activity"));
    }

    #[test]
    fn test_template_render_with_issues() {
        let config = Config::default();
        let template = ReportTemplate::new(&config);

        let mut activities = BTreeMap::new();
        let mut repo_activity = RepoActivity::default();

        repo_activity.new_issues.push(Issue {
            number: 42,
            title: "Test Issue".to_string(),
            body: None,
            state: IssueState::Open,
            author: Author {
                login: "testuser".to_string(),
                user_type: None,
            },
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            labels: vec![Label {
                name: "bug".to_string(),
                color: Some("red".to_string()),
                description: None,
            }],
            url: "https://github.com/test/repo/issues/42".to_string(),
            comments: CommentCount { total_count: 0 },
            is_pull_request: false,
        });

        activities.insert("test/repo".to_string(), repo_activity);

        let now = Timestamp::now();
        let since = now - 24_i64.hours();

        let result = template.render(&activities, since, now, &[]).unwrap();
        assert!(result.contains("test/repo"));
        assert!(result.contains("Test Issue"));
        assert!(result.contains("#42"));
        assert!(result.contains("`bug`"));
    }
}
