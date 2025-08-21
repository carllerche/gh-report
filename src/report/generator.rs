use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use std::collections::BTreeMap;
use tracing::{info, warn};

use super::{group_activities_by_repo, Report, ReportTemplate};
use crate::cache::{generate_cache_key, CacheManager};
use crate::claude::prompts::{generate_title_prompt, summarize_activities_prompt, system_prompt};
use crate::claude::{
    estimate_cost, estimate_tokens, resolve_model_alias, ClaudeInterface, Message, MessagesRequest,
};
use crate::config::Config;
use crate::github::{GitHubClient, Issue};
use crate::intelligence::IntelligentAnalyzer;
use crate::progress::ProgressReporter;
use crate::state::State;

pub struct ReportGenerator<'a> {
    github_client: GitHubClient,
    claude_client: Option<ClaudeInterface>,
    config: &'a Config,
    state: &'a State,
    cache_manager: Option<CacheManager>,
}

impl<'a> ReportGenerator<'a> {
    pub fn new(github_client: GitHubClient, config: &'a Config, state: &'a State) -> Self {
        // Try to create Claude client based on config
        let claude_client = match ClaudeInterface::new(&config.claude) {
            Ok(client) => client,
            Err(e) => {
                warn!("Failed to initialize Claude: {}", e);
                None
            }
        };

        // Initialize cache manager if caching is enabled
        let cache_manager = if config.cache.enabled {
            let cache_dir = dirs::cache_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("gh-report");

            let manager = CacheManager::new(
                cache_dir,
                config.cache.ttl_hours,
                config.cache.compression_enabled,
            );

            // Initialize cache directories
            if let Err(e) = manager.initialize() {
                warn!("Failed to initialize cache: {}", e);
                None
            } else {
                info!("Cache initialized with {} hour TTL", config.cache.ttl_hours);
                Some(manager)
            }
        } else {
            None
        };

        ReportGenerator {
            github_client,
            claude_client,
            config,
            state,
            cache_manager,
        }
    }

    pub fn generate(&self, lookback_days: u32) -> Result<Report> {
        self.generate_with_progress(lookback_days, false)
    }

    pub fn generate_with_progress(&self, lookback_days: u32, dry_run: bool) -> Result<Report> {
        let mut progress = ProgressReporter::new();
        let now = Timestamp::now();
        let since = now - (lookback_days as i64 * 24).hours();

        if !progress.is_interactive() {
            info!("Generating report for the last {} days", lookback_days);
            info!(
                "Fetching activity since {}",
                since.strftime("%Y-%m-%d %H:%M")
            );
        }

        if dry_run {
            info!("DRY RUN: Showing what would be fetched without generating report");
        }

        // Check if we have any repositories to report on
        let total_repos = self.state.tracked_repos.len() + self.config.repos.len();
        if total_repos == 0 {
            warn!("No repositories configured or tracked - report will be empty");
            warn!(
                "Config repos: {}, State tracked repos: {}",
                self.config.repos.len(),
                self.state.tracked_repos.len()
            );
            warn!("Run 'gh-report init' to discover repositories or add them to the config file");
        }

        let mut all_issues = Vec::new();
        let mut errors = Vec::new();

        // Collect all repositories to process (from both state and config)
        let mut repos_to_process = Vec::new();

        // Add tracked repos from state
        for (repo_name, _) in &self.state.tracked_repos {
            repos_to_process.push(repo_name.clone());
        }

        // Add repos from config
        for repo_config in &self.config.repos {
            if !repos_to_process.contains(&repo_config.name) {
                repos_to_process.push(repo_config.name.clone());
            }
        }

        // Start main progress bar
        let total_repos = repos_to_process.len();
        let _main_pb = progress.start_report_generation(total_repos);

        for repo_name in &repos_to_process {
            let repo_pb = progress.start_repo_fetch(repo_name);

            // Try cache first if available
            let cache_key =
                generate_cache_key(&["issues", repo_name, &since.as_millisecond().to_string()]);

            let cached_issues = if let Some(ref cache) = self.cache_manager {
                match cache.get_github_response(&cache_key) {
                    Ok(Some(data)) => match serde_json::from_slice::<Vec<Issue>>(&data) {
                        Ok(issues) => {
                            if !progress.is_interactive() {
                                info!(
                                    "  Using cached data for {} ({} issues)",
                                    repo_name,
                                    issues.len()
                                );
                            }
                            Some(issues)
                        }
                        Err(e) => {
                            warn!("Failed to deserialize cached issues: {}", e);
                            None
                        }
                    },
                    Ok(None) => None,
                    Err(e) => {
                        warn!("Cache read error: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            let issues = if let Some(cached) = cached_issues {
                cached
            } else {
                // Fetch from GitHub
                match self.github_client.fetch_issues(repo_name, Some(since)) {
                    Ok(mut issues) => {
                        issues.retain(|issue| issue.updated_at >= since);

                        if !progress.is_interactive() {
                            info!("  Found {} active issues/PRs", issues.len());
                        }

                        // Cache the result (unless dry run)
                        if !dry_run {
                            if let Some(ref cache) = self.cache_manager {
                                let data = serde_json::to_vec(&issues).unwrap_or_default();
                                if let Err(e) = cache.cache_github_response(&cache_key, &data) {
                                    warn!("Failed to cache GitHub response: {}", e);
                                }
                            }
                        }

                        issues
                    }
                    Err(e) => {
                        let error_msg = format!("{}", e);
                        progress.report_repo_error(repo_pb.as_ref(), repo_name, &error_msg);
                        warn!("Failed to fetch issues for {}: {}", repo_name, e);
                        errors.push(format!("⚠️ Could not fetch data for {}: {}", repo_name, e));
                        continue;
                    }
                }
            };

            progress.complete_repo_fetch(repo_pb.as_ref(), repo_name, issues.len());
            all_issues.extend(issues);
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
                        errors.push(format!(
                            "⚠️ Could not fetch mentions for {}: {}",
                            username, e
                        ));
                    }
                }
            }
        }

        // Stop here if dry run
        if dry_run {
            info!("\nDRY RUN Summary:");
            info!("  Total repositories: {}", self.state.tracked_repos.len());
            info!("  Total items found: {}", all_issues.len());
            info!("  Errors encountered: {}", errors.len());

            let activities = group_activities_by_repo(all_issues);
            for (repo, activity) in &activities {
                let total = activity.new_issues.len()
                    + activity.updated_issues.len()
                    + activity.new_prs.len()
                    + activity.updated_prs.len();
                if total > 0 {
                    info!("  {}: {} items", repo, total);
                }
            }

            // Return empty report for dry run
            return Ok(Report {
                title: "Dry Run - No Report Generated".to_string(),
                content: String::new(),
                timestamp: now,
                estimated_cost: 0.0,
            });
        }

        // Group activities and run analysis for actual report generation
        let activities = group_activities_by_repo(all_issues);

        // Apply intelligent analysis
        let analyzer = IntelligentAnalyzer::new(&self.config);
        let analysis = analyzer.analyze(&activities);

        info!(
            "Intelligent analysis: {} prioritized items, {} action items",
            analysis.prioritized_issues.len(),
            analysis.action_items.len()
        );

        // Generate AI summary if Claude is available
        let (ai_summary, ai_title, estimated_cost) = if let Some(claude) = &self.claude_client {
            let ai_pb = progress.start_ai_summary();
            // Include context from intelligent analysis
            let context_prompt = Some(analysis.context_prompt.as_str());
            match self.generate_ai_summary_with_context(claude, &activities, context_prompt) {
                Ok((summary, title, cost)) => {
                    progress.complete_ai_summary(ai_pb.as_ref(), cost);
                    if !progress.is_interactive() {
                        info!("Generated AI summary (estimated cost: ${:.4})", cost);
                    }
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
        self.github_client
            .fetch_mentions(since)
            .context("Failed to fetch user mentions")
    }

    fn generate_ai_summary(
        &self,
        claude: &ClaudeInterface,
        activities: &BTreeMap<String, crate::github::RepoActivity>,
    ) -> Result<(String, String, f32)> {
        self.generate_ai_summary_with_context(claude, activities, None)
    }

    fn generate_ai_summary_with_context(
        &self,
        claude: &ClaudeInterface,
        activities: &BTreeMap<String, crate::github::RepoActivity>,
        context: Option<&str>,
    ) -> Result<(String, String, f32)> {
        // Generate the prompt
        let prompt = summarize_activities_prompt(activities, context);

        // Generate cache key for this prompt
        let prompt_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(prompt.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        let cache_key = generate_cache_key(&[
            "claude_summary",
            &prompt_hash[..16], // Use first 16 chars of hash
        ]);

        // Try to get from cache
        if let Some(ref cache) = self.cache_manager {
            if let Ok(Some(cached)) = cache.get_claude_response(&cache_key) {
                // Parse cached response (format: "TITLE\n---\nSUMMARY\n---\nCOST")
                let parts: Vec<&str> = cached.split("\n---\n").collect();
                if parts.len() == 3 {
                    let title = parts[0].to_string();
                    let summary = parts[1].to_string();
                    let cost: f32 = parts[2].parse().unwrap_or(0.0);
                    info!("Using cached AI summary (saved cost: ${:.4})", cost);
                    return Ok((summary, title, 0.0)); // Return 0 cost since we didn't call API
                }
            }
        }

        // Estimate tokens
        let input_tokens = estimate_tokens(&prompt) + estimate_tokens(&system_prompt());

        // Create request
        let model = resolve_model_alias(&self.config.claude.primary_model);
        let request = MessagesRequest::new(model.clone(), vec![Message::user(prompt)])
            .with_system(system_prompt())
            .with_max_tokens(4000);

        // Send request
        let response = match claude.messages(request) {
            Ok(resp) => resp,
            Err(e) => {
                // Log the actual error for debugging
                warn!("Claude API error details: {:#}", e);

                let error_str = e.to_string();

                // Provide helpful error messages based on the error type
                if error_str.contains("ANTHROPIC_API_KEY") {
                    return Err(anyhow::anyhow!("ANTHROPIC_API_KEY environment variable is not set. Please set it to use AI summarization."));
                } else if error_str.contains("invalid x-api-key")
                    || error_str.contains("authentication_error")
                {
                    return Err(anyhow::anyhow!("Invalid ANTHROPIC_API_KEY. Please check that your API key is correct and active."));
                } else if error_str.contains("rate_limit") {
                    return Err(anyhow::anyhow!(
                        "Claude API rate limit exceeded. Please try again later."
                    ));
                } else if error_str.contains("overloaded") {
                    return Err(anyhow::anyhow!(
                        "Claude API is currently overloaded. Please try again in a few moments."
                    ));
                }

                return Err(e).context("Failed to get summary from Claude");
            }
        };

        let summary = response.get_text();
        let output_tokens = response.usage.output_tokens;

        // Generate title from summary
        let title_prompt = generate_title_prompt(&summary);
        let title_request = MessagesRequest::new(
            resolve_model_alias(&self.config.claude.secondary_model),
            vec![Message::user(title_prompt)],
        )
        .with_max_tokens(100);

        let title_response = claude
            .messages(title_request)
            .context("Failed to generate title from Claude")?;

        let title = title_response.get_text().trim().to_string();

        // Calculate total cost
        let summary_cost = estimate_cost(&model, input_tokens, output_tokens);
        let title_cost = estimate_cost(
            &self.config.claude.secondary_model,
            estimate_tokens(&generate_title_prompt(&summary)),
            title_response.usage.output_tokens,
        );

        let total_cost = summary_cost + title_cost;

        // Cache the result
        if let Some(ref cache) = self.cache_manager {
            let cached_data = format!("{}\n---\n{}\n---\n{}", title, summary, total_cost);
            if let Err(e) = cache.cache_claude_response(&cache_key, &cached_data) {
                warn!("Failed to cache Claude response: {}", e);
            }
        }

        Ok((summary, title, total_cost))
    }

    fn generate_title(
        &self,
        since: Timestamp,
        now: Timestamp,
        activities: &BTreeMap<String, crate::github::RepoActivity>,
    ) -> String {
        let date_range =
            if since.strftime("%Y-%m-%d").to_string() == now.strftime("%Y-%m-%d").to_string() {
                format!("Daily Report - {}", now.strftime("%Y-%m-%d"))
            } else {
                format!(
                    "Report - {} to {}",
                    since.strftime("%Y-%m-%d"),
                    now.strftime("%Y-%m-%d")
                )
            };

        let total_items: usize = activities
            .values()
            .map(|a| {
                a.new_issues.len() + a.updated_issues.len() + a.new_prs.len() + a.updated_prs.len()
            })
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
