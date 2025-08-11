# Github Report

A command-line tool that generates intelligent daily reports of GitHub activity, providing personalized summaries and actionable insights for maintainers managing multiple repositories.

## Executive Summary

**Purpose**: Automate GitHub activity monitoring and summarization for maintainers of multiple open source projects, filtering noise and highlighting critical decisions.

**Key Value**: Reduces time spent monitoring GitHub from hours to minutes by using AI to intelligently filter, summarize, and prioritize activity across dozens of repositories.

## Problem Statement

### Current Challenges
- **Volume Overload**: Maintainers of popular open source projects receive hundreds of notifications daily across issues, PRs, and discussions
- **Signal vs Noise**: 90% of activity doesn't require maintainer intervention due to active community involvement
- **Context Switching**: Managing projects at different lifecycle stages (prototype, pre-release, mature, deprecated) requires different attention levels
- **Critical Visibility**: Important changes (API breaks, security issues, architectural decisions) can be buried in routine activity

### User Needs
1. **Selective Awareness**: Need visibility into API changes, breaking changes, and architectural decisions without reading every issue
2. **Actionable Intelligence**: Clear identification of items requiring personal response vs community-handled items
3. **Contextual Prioritization**: Different repositories require different monitoring strategies based on role (owner, maintainer, contributor)
4. **Time Efficiency**: Reduce daily GitHub review from 1-2 hours to a 5-minute report scan

## Implementation Specification

### Technology Stack
- **Language**: Rust
- **GitHub Integration**: Via `gh` CLI (handles authentication)
- **AI Integration**: Claude API for summarization and intelligent filtering
- **Configuration**: TOML format
- **Report Output**: Markdown files

### Rust Implementation Details

#### Claude API Client
Since there's no official Anthropic Rust SDK, we'll implement a lightweight HTTP client:

**Dependencies**:
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
```

**Basic API Client Structure**:
```rust
pub struct ClaudeClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl ClaudeClient {
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.client
            .post(&format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?
            .json()
            .await
    }
    
    /// Resolves model aliases to specific model versions
    pub fn resolve_model(alias: &str) -> &'static str {
        match alias {
            "sonnet" => "claude-3-5-sonnet-20241022",
            "haiku" => "claude-3-5-haiku-20241022", 
            "opus" => "claude-3-opus-20240229",  // If/when available
            specific => specific,  // Pass through specific model names
        }
    }
}
```

**Alternative Option**: Use an existing community crate if available and well-maintained:
- `anthropic-rs` (if actively maintained)
- `claude-rs` (if exists and suitable)

The implementation will use an enum-based abstraction pattern:
```rust
pub enum Claude {
    Real(RealClient),
    #[cfg(test)]
    Mock(MockClient),
}

impl Claude {
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        match self {
            Claude::Real(client) => client.complete(request).await,
            #[cfg(test)]
            Claude::Mock(client) => client.complete(request).await,
        }
    }
}
```

This pattern avoids unnecessary trait indirection and makes the code more explicit about available implementations.

### CLI Commands

#### `gh-report`
Main command to generate a report. Analyzes activity since the last report was generated.

**Options:**
- `--config <path>` - Path to configuration file (default: `~/.gh-report.toml`)
- `--since <date>` - Override the automatic date detection
- `--output <path>` - Override the output file location
- `--dry-run` - Preview what would be fetched without generating report
- `--estimate-cost` - Show estimated Claude API cost before proceeding
- `--no-cache` - Bypass cache and fetch fresh data from all sources
- `--clear-cache` - Clear all cached data before running

#### `gh-report init`
Analyzes the user's GitHub activity over the past 30 days and generates an initial configuration file with:
- Repositories the user has recently contributed to (commits, PRs, issues, comments)
- Suggested labels based on patterns (e.g., "rust-libs", "personal-tools", "oss-contributing")
- Default watch rules for each label
- Weighted by activity type: commits (4x) > PRs (3x) > issues (2x) > comments (1x)

**Options:**
- `--lookback <days>` - Number of days to analyze (default: 30)
- `--config <path>` - Where to write the configuration file

#### `gh-report rebuild-state`
Rebuilds the state file by scanning existing reports in the report directory.

### CLI User Experience

#### Progress Indicators
The tool should provide clear, polished feedback during execution:

```
$ gh-report
✓ Loading configuration
✓ Checking last report: 2024-01-14 (1 day ago)
⠋ Fetching GitHub activity...
  └─ Repositories: 23/45
  └─ Issues/PRs: 89 found
⠸ Analyzing with Claude...
  └─ Processing high priority items (3/5)
✓ Report generated: ~/Github Reports/2024-01-15 - Github - Tokio API Changes.md
  
Summary: 12 items need attention, 3 require action
Estimated Claude API cost: $0.03
```

**Implementation approach**:
- Use a crate like `indicatif` for progress bars and spinners
- Start simple with basic status messages, evolve to rich progress indicators
- Suppress progress output when piped or in CI environments (check `isatty`)

#### Dry Run Mode
When using `--dry-run`, show what would be done without making Claude API calls:

```
$ gh-report --dry-run
✓ Configuration loaded
✓ Last report: 2024-01-14 09:30:00

Would check 42 repositories:
  High priority (5):
    - tokio-rs/tokio (activity score: 89)
    - tokio-rs/axum (activity score: 45)
    ...
  
  Medium priority (12):
    - rust-lang/rust (mentions only)
    ...
    
  Low priority (25):
    - personal projects and archived repos
    
Would fetch:
  - Issues/PRs from last 24 hours
  - Comments on open issues/PRs
  - Mentions across GitHub
  
Estimated:
  - GitHub API calls: ~150
  - Claude API calls: 3-5
  - Estimated cost: $0.02-0.04
  - Estimated time: 30-45 seconds
```

### Configuration Structure

**Location**: `~/.gh-report.toml`

```toml
# Core settings
[settings]
report_dir = "~/Github Reports"  # Where reports are saved
state_file = "~/Github Reports/.gh-report-state.json"  # Tracks last run
file_name_format = "{yyyy-mm-dd} - Github - {short-title}"
max_lookback_days = 30  # Maximum days to look back if tool hasn't run
max_issues_per_report = 100  # Data limits
max_comments_per_report = 500
inactive_repo_threshold_days = 30  # When to stop watching inactive repos

# Claude API Configuration
[claude]
# Authentication methods (checked in order of precedence):
# 1. ANTHROPIC_API_KEY environment variable (recommended)
# 2. API key helper command (for dynamic/rotating keys)
# 3. Config file api_key field (not recommended - security risk)
# 
# api_key = "sk-ant-..."  # Only use for testing, prefer env var
# api_key_helper = "~/.gh-report/get-api-key.sh"  # Script that outputs API key

# Model selection - can use aliases or specific versions
# Aliases: "sonnet" (latest), "haiku" (latest), "opus" (if available)
# Specific: "claude-3-5-sonnet-20241022"
primary_model = "sonnet"  # For important sections (auto-selects latest)
secondary_model = "haiku"  # For FYI sections (auto-selects latest)
# primary_model = "claude-3-5-sonnet-20241022"  # Pin to specific version
cache_responses = true
cache_ttl_hours = 24

# Report template configuration
[report]
template = """
# GitHub Activity Report - {date}

## 🚨 Action Required
{action_required}

## 👀 Needs Attention
{needs_attention}

## 📋 Key Changes and Proposals
{key_changes}

## 💡 Suggested Actions
{suggested_actions}

## 📰 FYI
{fyi}

## 📊 Repository Activity Changes
{repo_changes}

---
*Report generated at {timestamp} | Est. cost: ${cost}*
"""

# Labels define reusable watching patterns
[[labels]]
name = "rust-libs"
description = "Rust libraries I maintain"
watch_rules = [
  "api_changes",
  "breaking_changes",
  "security_issues"
]
importance = "high"
context = """
I maintain these Rust libraries and need to be aware of:
- Any API changes or breaking changes
- Performance regressions
- Security vulnerabilities
- Major feature proposals
"""

[[labels]]
name = "oss-contributing"
description = "Open source projects I contribute to"
watch_rules = [
  "mentions",
  "review_requests"
]
importance = "medium"
context = """
Projects where I'm an active contributor but not maintainer.
Focus on PRs that need my review or issues where I'm mentioned.
"""

[[labels]]
name = "personal-tools"
description = "Personal tools and experiments"
watch_rules = ["all_activity"]
importance = "low"
context = "Personal projects - summarize any external contributions"

# Repository configurations
[[repos]]
name = "tokio-rs/tokio"
labels = ["rust-libs", "async-runtime"]
watch_rules = ["api_changes", "performance", "breaking_changes"]
importance_override = "critical"  # Overrides label importance
custom_context = """
Core async runtime for Rust. Critical to monitor:
- API stability
- Performance characteristics
- Ecosystem compatibility
"""

[[repos]]
name = "carllerche/my-tool"
labels = ["personal-tools"]
# Inherits watch rules from label

# Dynamic repository detection rules
[dynamic_repos]
enabled = true
auto_add_threshold_days = 7  # Add repo if active in last 7 days
auto_remove_threshold_days = 30  # Remove if inactive for 30 days
activity_weights = { commits = 4, prs = 3, issues = 2, comments = 1 }
min_activity_score = 5  # Minimum weighted score to auto-add

# Watch rules define what to track
[watch_rules]
api_changes = ["public API", "breaking change", "deprecation", "new feature"]
breaking_changes = ["BREAKING", "migration", "major version"]
security_issues = ["security", "vulnerability", "CVE", "exploit"]
performance = ["performance", "regression", "benchmark", "slow"]
mentions = ["@{username}"]
review_requests = ["review requested", "PTAL", "feedback needed"]
all_activity = []  # Watch everything
```

### State Management

**State file format** (`~/Github Reports/.gh-report-state.json`):
```json
{
  "last_run": "2024-01-15T09:30:00Z",
  "last_report_file": "2024-01-15 - Github - API Changes in Tokio.md",
  "tracked_repos": {
    "tokio-rs/tokio": {
      "last_seen": "2024-01-15T09:30:00Z",
      "activity_score": 45,
      "auto_tracked": false
    },
    "rust-lang/rust": {
      "last_seen": "2024-01-14T15:20:00Z",
      "activity_score": 12,
      "auto_tracked": true
    }
  }
}
```

### API Response Caching

To avoid duplicate API calls and support interruption recovery, cache API responses in a dedicated directory:

**Cache location**: `~/Github Reports/.cache/`

**Cache structure**:
```
.cache/
├── github/
│   ├── 2024-01-15/
│   │   ├── issues-tokio-rs-tokio.json
│   │   ├── prs-tokio-rs-tokio.json
│   │   └── comments-issue-6234.json
│   └── 2024-01-14/
│       └── ...
└── claude/
    ├── summaries/
    │   ├── issue-6234-hash.json  # Hash of content for cache key
    │   └── pr-2145-hash.json
    └── suggestions/
        └── ...
```

**Caching behavior**:
- GitHub responses: Cache for current day only (cleared at midnight)
- Claude responses: Cache for 24 hours based on content hash
- On interruption (Ctrl-C): 
  - Cancel in-flight requests immediately
  - Preserve all cached responses
  - Next run resumes using cached data
- Cache cleanup: Remove files older than 7 days on each run
- CLI overrides:
  - `--no-cache`: Ignores existing cache but still writes new responses to cache
  - `--clear-cache`: Deletes entire cache directory before starting

**Implementation notes**:
- Use `sha256` hash of request content as cache key for Claude
- Store cache with compression (`flate2` crate) to save disk space
- Validate cache entries with timestamps before use

### GitHub Data Collection

#### Data Sources
1. **Repositories**:
   - Explicitly configured repos
   - Dynamically detected based on user activity
   - New repos (created by user or added as collaborator)
   - Starred repositories (optional section)

2. **Activity Types**:
   - Issues (new, closed, commented)
   - Pull requests (new, merged, closed, reviewed)
   - Discussions (if enabled)
   - Releases
   - Security advisories
   - User mentions across GitHub

#### Fetching Strategy
```bash
# Pseudo-code for data fetching via gh CLI
gh api search/issues --params "involves:@me updated:>$last_run"
gh api users/$user/repos --params "type:all sort:pushed"
gh api notifications --params "all:true since:$last_run"
```

### Report Generation Process

1. **Data Collection Phase**:
   - Fetch all relevant GitHub data since last run
   - Apply watch rules and filters
   - Score items by importance

2. **Claude Processing Phase**:
   - **Batch 1** (Critical/High importance) → Primary model:
     - Generate detailed summaries
     - Extract action items
     - Suggest responses for issues requiring user action
   - **Batch 2** (Medium importance) → Primary model:
     - Moderate detail summaries
     - Highlight key decisions needed
   - **Batch 3** (Low importance/FYI) → Secondary model:
     - One-line summaries
     - Bulk processing for efficiency

3. **Report Assembly**:
   - Apply markdown template
   - Order by importance score
   - Generate short title from content
   - Calculate and display API costs

### Claude Integration Strategy

#### Authentication Methods
The tool supports multiple authentication methods for the Anthropic API, checked in this order:

1. **Environment Variable** (Recommended):
   ```bash
   export ANTHROPIC_API_KEY="sk-ant-api03-..."
   gh-report
   ```

2. **API Key Helper Script** (For dynamic/rotating keys):
   ```toml
   [claude]
   api_key_helper = "~/.gh-report/get-api-key.sh"
   ```
   The helper script should output the API key to stdout. This allows integration with:
   - Corporate credential management systems
   - Cloud provider secret managers (AWS Secrets Manager, etc.)
   - Password managers (1Password CLI, etc.)
   
   Example helper script:
   ```bash
   #!/bin/bash
   # get-api-key.sh
   op item get "Anthropic API" --fields credential  # 1Password example
   ```

3. **Configuration File** (Not recommended - security risk):
   ```toml
   [claude]
   api_key = "sk-ant-api03-..."
   ```

If no API key is found, the tool will display:
- Instructions for obtaining an API key from console.anthropic.com
- Examples of each configuration method
- Security best practices for API key storage

#### Prompt Templates

**For Action Required Section**:
```
You are analyzing GitHub activity for a repository maintainer. 
Context: {repo_context}
Activity: {activity_data}

Provide:
1. A 1-2 sentence summary of what requires action
2. Why this needs the maintainer's attention
3. A suggested response or action (be specific)

Format as markdown. Be concise but complete.
```

**For Summarization**:
```
Summarize the following GitHub activity for a {importance} priority repository.
Repository context: {custom_context}
Watch rules active: {watch_rules}

Activity:
{activity_list}

Provide a {detail_level} summary focusing on:
- Changes matching watch rules
- Decisions that need to be made
- Notable discussions or proposals

Output {max_length} words maximum.
```

#### Caching Strategy
- Cache summaries for identical issue/PR content (24 hour TTL)
- Cache suggested responses for similar issue patterns
- Store cache in state file to persist between runs

### Example Output Report

```markdown
# GitHub Activity Report - 2024-01-15

## 🚨 Action Required

### tokio-rs/tokio #6234 - Breaking Change Proposal: Remove deprecated runtime APIs
**Requires your approval** - PR proposes removing deprecated runtime builder APIs that were marked for removal in 2.0. This affects a significant portion of the ecosystem.

**Suggested response**: Approve with migration guide requirement. Comment: "LGTM for 2.0, but we need a comprehensive migration guide. Can you add examples for the most common patterns?"

### carllerche/mini-redis #89 - Security: Potential DoS via unbounded channel
**Security vulnerability** - User reported potential DoS vector through unbounded channel in connection handler.

**Suggested action**: Acknowledge immediately and create patch. This is a valid issue that needs a 0.4.1 patch release with bounded channels.

## 👀 Needs Attention

### tokio-rs/tokio #6229 - Discussion: async trait stabilization impact
The stabilization of async traits in Rust 1.75 opens opportunities for API improvements. Community discussing whether to adopt immediately or wait. **Key decision**: Should Tokio 1.x adopt async traits in a minor version or wait for 2.0?

### rust-lang/rust #119876 - Regression in async performance
Performance regression reported in latest nightly affecting Tokio benchmarks (-15% throughput). Being investigated by compiler team, may need Tokio team input on test cases.

## 📋 Key Changes and Proposals

### tokio-rs/axum 0.7.3 Released
- Fixed middleware ordering bug (#2145)
- Added new `RouterExt` trait for better ergonomics
- **Breaking**: Changed default timeout from 30s to 60s

### hyperium/hyper - Moving to sans-io design
Major architectural shift proposed for hyper 2.0, moving to sans-io pattern. Would require significant changes in Tokio integration layer. Community feedback period open until Feb 1.

## 💡 Suggested Actions

1. **Create migration guide** for the deprecated Tokio runtime APIs before approving #6234
2. **Schedule security release** for mini-redis DoS fix - aim for this week
3. **Comment on async trait discussion** with your position on adoption timeline

## 📰 FYI

- `tokio-rs/bytes`: 3 new contributors, mostly documentation improvements
- `carllerche/tower`: Dependabot updates merged
- `rust-lang/futures-rs`: Discussion on futures 0.4 roadmap started
- `tokio-rs/tracing`: Performance improvements in latest release (+10% throughput)

## 📊 Repository Activity Changes

### Newly Tracked
- **rust-lang/polonius** - You commented on RFC discussion (auto-added)

### Becoming Inactive (will be removed in 7 days)
- **old-project/archived-tool** - No activity for 23 days

---
*Report generated at 2024-01-15 09:30:00 PST | Est. cost: $0.03*
```

### Data Limits and Error Handling

#### Rate Limiting
- Respect GitHub API rate limits (5000 req/hour authenticated)
- Implement exponential backoff for Claude API
- Cache aggressively to minimize API calls

#### Data Overload Handling
When exceeding limits (100 issues or 500 comments):
1. Priority scoring based on:
   - Repository importance
   - Watch rule matches
   - User involvement (author > mentioned > participant)
   - Recency
2. Include notice in report: "⚠️ High activity period - showing top 100 items. Full activity: 234 issues, 892 comments"
3. Focus on items needing action over informational items

#### Error Recovery
- Network failures: Retry with exponential backoff
- Partial data: Generate report with available data, note missing sources
- Claude API failures: Fatal error (core feature)
- Invalid configuration: Detailed error messages with examples

### Testing Strategy

#### Unit Tests
- **Configuration parsing**: Test TOML parsing, validation, and defaults
- **Watch rules engine**: Test pattern matching and scoring logic
- **Date/time handling**: Test report scheduling and lookback periods using `jiff`
- **Template rendering**: Test variable substitution and markdown generation

#### Integration Tests
- **GitHub CLI interaction**: 
  ```rust
  enum GitHubClient {
      Real(RealGitHub),
      #[cfg(test)]
      Mock(MockGitHub),  // Returns canned responses
  }
  ```
- **Claude API**: Use `wiremock` to mock HTTP responses
- **End-to-end**: Generate test reports from fixture data

#### Snapshot Testing with `insta`
- Capture generated reports as snapshots
- Verify markdown formatting consistency
- Track changes in report structure
- Example:
  ```rust
  #[test]
  fn test_report_generation() {
      let report = generate_report(test_data);
      insta::assert_snapshot!(report);
  }
  ```

#### Test Data Strategy
- Fixture files with sample GitHub API responses
- Mock Claude responses for different scenarios
- Test edge cases:
  - Empty activity periods
  - Rate limit handling
  - Malformed API responses
  - Large data volumes exceeding limits

### Development Phases

#### Phase 1: Core Functionality
- Basic CLI structure with `clap`
- Configuration parsing with `serde` and `toml`
- GitHub data fetching via `gh` CLI using `std::process::Command`
- Simple report generation to markdown

#### Phase 2: Claude Integration
- Implement Claude HTTP API client with `reqwest`
- Add request/response types with `serde`
- Intelligent summarization with prompt engineering
- Action suggestions
- Cost tracking based on token usage

#### Phase 3: Dynamic Repos & Labels
- Auto-detection of repositories from GitHub activity
- Label system implementation
- Watch rules engine with pattern matching

#### Phase 4: Polish
- Template customization with simple variable substitution
- Caching layer using `serde_json` for persistence
- Performance optimization with concurrent API calls using `tokio`
- Comprehensive error handling with `anyhow` or `thiserror`

### Concurrency Strategy

**Note**: Initial implementation should be synchronous for simplicity. Concurrency optimization comes in Phase 4.

#### Design Considerations (for future design document)
1. **GitHub API Concurrency**:
   - GitHub allows up to 5000 requests/hour for authenticated users
   - Safe concurrent limit: 10-20 parallel requests
   - Use semaphore to limit concurrent `gh` subprocess spawns
   
2. **Claude API Concurrency**:
   - Batch by importance level (high, medium, low)
   - Process items within same importance level concurrently
   - Reasonable limit: 3-5 concurrent Claude requests
   
3. **Implementation approach**:
   ```rust
   // Future concurrent implementation sketch
   use tokio::sync::Semaphore;
   
   const MAX_GITHUB_CONCURRENT: usize = 15;
   const MAX_CLAUDE_CONCURRENT: usize = 5;
   
   let github_semaphore = Arc::new(Semaphore::new(MAX_GITHUB_CONCURRENT));
   let claude_semaphore = Arc::new(Semaphore::new(MAX_CLAUDE_CONCURRENT));
   ```

4. **Benefits of async/concurrent approach**:
   - Reduce total runtime from ~60s to ~10-15s for typical report
   - Better resource utilization
   - Improved user experience with parallel progress indicators

5. **Challenges to address in design doc**:
   - Rate limit handling with exponential backoff
   - Coordinating progress reporting across concurrent tasks
   - Error aggregation and partial failure handling
   - Memory usage with large concurrent responses

### Key Rust Dependencies

```toml
[dependencies]
# CLI and configuration
clap = { version = "4", features = ["derive", "env"] }
toml = "0.8"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Async runtime and HTTP
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Error handling and utilities
anyhow = "1.0"
jiff = { version = "0.2", features = ["serde"] }  # Modern date/time handling
regex = "1.10"
dirs = "5.0"  # For home directory expansion

# Progress indicators (Phase 2+)
indicatif = "0.17"  # Progress bars and spinners
atty = "0.2"  # Detect if stdout is a terminal

# Caching
sha2 = "0.10"  # For content hashing
flate2 = "1.0"  # Compression for cache files

# Templating (if needed)
handlebars = "5.0"  # Optional, for advanced templating

# Testing
[dev-dependencies]
insta = { version = "1.34", features = ["json", "yaml"] }  # Snapshot testing
tempfile = "3.8"  # Temporary directories for tests
wiremock = "0.6"  # Mock HTTP server for API testing
```