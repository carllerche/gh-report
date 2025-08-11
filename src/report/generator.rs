use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use tracing::{info, warn};
use std::collections::BTreeMap;

use crate::config::Config;
use crate::github::{GitHubClient, Issue};
use crate::state::State;
use crate::claude::{ClaudeClient, MessagesRequest, Message, resolve_model_alias, estimate_tokens, estimate_cost};
use crate::claude::prompts::{system_prompt, summarize_activities_prompt, generate_title_prompt};
use crate::intelligence::IntelligentAnalyzer;
use super::{Report, ReportTemplate, group_activities_by_repo};

pub struct ReportGenerator<'a> {
    github_client: GitHubClient,
    claude_client: Option<ClaudeClient>,
    config: &'a Config,
    state: &'a State,
}

impl<'a> ReportGenerator<'a> {
    pub fn new(github_client: GitHubClient, config: &'a Config, state: &'a State) -> Self {
        // Try to create Claude client if API key is available
        let claude_client = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(_) => match ClaudeClient::new() {
                Ok(client) => {
                    info!("Claude API client initialized");
                    Some(client)
                }
                Err(e) => {
                    warn!("Failed to initialize Claude client: {}", e);
                    None
                }
            },
            Err(_) => {
                info!("ANTHROPIC_API_KEY not set, running without AI summarization");
                None
            }
        };
        
        ReportGenerator { 
            github_client, 
            claude_client,
            config, 
            state 
        }
    }

    pub fn generate(&self, lookback_days: u32) -> Result<Report> {
        let now = Timestamp::now();
        let since = now - (lookback_days as i64 * 24).hours();
        
        info!("Generating report for the last {} days", lookback_days);
        info!("Fetching activity since {}", since.strftime("%Y-%m-%d %H:%M"));

        let mut all_issues = Vec::new();
        let mut errors = Vec::new();

        for (repo_name, _repo_state) in &self.state.tracked_repos {
            info!("Fetching issues for {}", repo_name);
            
            match self.github_client.fetch_issues(repo_name, Some(since)) {
                Ok(mut issues) => {
                    issues.retain(|issue| issue.updated_at >= since);
                    
                    info!("  Found {} active issues/PRs", issues.len());
                    all_issues.extend(issues);
                }
                Err(e) => {
                    warn!("Failed to fetch issues for {}: {}", repo_name, e);
                    errors.push(format!("⚠️ Could not fetch data for {}: {}", repo_name, e));
                }
            }
        }

        // TODO: Add include_mentions configuration option
        let include_mentions: Vec<String> = vec![];
        if !include_mentions.is_empty() {
            info!("Fetching mentions for users: {:?}", include_mentions);
            
            for username in &include_mentions {
                match self.fetch_user_mentions(username, since) {
                    Ok(mut mentions) => {
                        info!("  Found {} mentions for {}", mentions.len(), username);
                        all_issues.append(&mut mentions);
                    }
                    Err(e) => {
                        warn!("Failed to fetch mentions for {}: {}", username, e);
                        errors.push(format!("⚠️ Could not fetch mentions for {}: {}", username, e));
                    }
                }
            }
        }

        let activities = group_activities_by_repo(all_issues);
        
        // Apply intelligent analysis
        let analyzer = IntelligentAnalyzer::new(&self.config);
        let analysis = analyzer.analyze(&activities);
        
        info!("Intelligent analysis: {} prioritized items, {} action items", 
            analysis.prioritized_issues.len(),
            analysis.action_items.len());
        
        // Generate AI summary if Claude is available
        let (ai_summary, ai_title, estimated_cost) = if let Some(claude) = &self.claude_client {
            // Include context from intelligent analysis
            let context_prompt = Some(analysis.context_prompt.as_str());
            match self.generate_ai_summary_with_context(claude, &activities, context_prompt) {
                Ok((summary, title, cost)) => {
                    info!("Generated AI summary (estimated cost: ${:.4})", cost);
                    (Some(summary), Some(title), cost)
                }
                Err(e) => {
                    warn!("Failed to generate AI summary: {}", e);
                    errors.push(format!("⚠️ AI summarization failed: {}", e));
                    (None, None, 0.0)
                }
            }
        } else {
            (None, None, 0.0)
        };
        
        let template = ReportTemplate::new(&self.config);
        let content = template.render_with_intelligence(
            &activities,
            since,
            now,
            &errors,
            ai_summary.as_deref(),
            &analysis,
        )?;

        let title = ai_title.unwrap_or_else(|| self.generate_title(since, now, &activities));

        Ok(Report {
            title,
            content,
            timestamp: now,
            estimated_cost,
        })
    }

    fn fetch_user_mentions(&self, _username: &str, since: Timestamp) -> Result<Vec<Issue>> {
        self.github_client.fetch_mentions(since)
            .context("Failed to fetch user mentions")
    }
    
    fn generate_ai_summary(
        &self,
        claude: &ClaudeClient,
        activities: &BTreeMap<String, crate::github::RepoActivity>,
    ) -> Result<(String, String, f32)> {
        self.generate_ai_summary_with_context(claude, activities, None)
    }
    
    fn generate_ai_summary_with_context(
        &self,
        claude: &ClaudeClient,
        activities: &BTreeMap<String, crate::github::RepoActivity>,
        context: Option<&str>,
    ) -> Result<(String, String, f32)> {
        // Generate the prompt
        let prompt = summarize_activities_prompt(activities, context);
        
        // Estimate tokens
        let input_tokens = estimate_tokens(&prompt) + estimate_tokens(&system_prompt());
        
        // Create request
        let model = resolve_model_alias(&self.config.claude.primary_model);
        let request = MessagesRequest::new(
            model.clone(),
            vec![Message::user(prompt)],
        )
        .with_system(system_prompt())
        .with_max_tokens(4000);
        
        // Send request
        let response = claude.messages(request)
            .context("Failed to get summary from Claude")?;
        
        let summary = response.get_text();
        let output_tokens = response.usage.output_tokens;
        
        // Generate title from summary
        let title_prompt = generate_title_prompt(&summary);
        let title_request = MessagesRequest::new(
            resolve_model_alias(&self.config.claude.secondary_model),
            vec![Message::user(title_prompt)],
        )
        .with_max_tokens(100);
        
        let title_response = claude.messages(title_request)
            .context("Failed to generate title from Claude")?;
        
        let title = title_response.get_text().trim().to_string();
        
        // Calculate total cost
        let summary_cost = estimate_cost(&model, input_tokens, output_tokens);
        let title_cost = estimate_cost(
            &self.config.claude.secondary_model,
            estimate_tokens(&generate_title_prompt(&summary)),
            title_response.usage.output_tokens,
        );
        
        Ok((summary, title, summary_cost + title_cost))
    }

    fn generate_title(&self, since: Timestamp, now: Timestamp, activities: &BTreeMap<String, crate::github::RepoActivity>) -> String {
        let date_range = if since.strftime("%Y-%m-%d").to_string() == now.strftime("%Y-%m-%d").to_string() {
            format!("Daily Report - {}", now.strftime("%Y-%m-%d"))
        } else {
            format!("Report - {} to {}", 
                since.strftime("%Y-%m-%d"),
                now.strftime("%Y-%m-%d"))
        };

        let total_items: usize = activities.values()
            .map(|a| a.new_issues.len() + a.updated_issues.len() + a.new_prs.len() + a.updated_prs.len())
            .sum();

        if total_items > 0 {
            format!("{} ({} items)", date_range, total_items)
        } else {
            date_range
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::MockGitHub;

    #[test]
    fn test_report_generator_creation() {
        let mock = MockGitHub::new();
        let github_client = GitHubClient::Mock(mock);
        let config = Config::default();
        let state = State::default();
        
        let generator = ReportGenerator::new(github_client, &config, &state);
        
        // Generate should work even without Claude client
        let result = generator.generate(1);
        assert!(result.is_ok());
    }
}