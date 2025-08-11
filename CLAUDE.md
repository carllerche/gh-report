# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a CLI tool called `gh-report` that generates daily summary reports of GitHub activity for users who manage multiple repositories. It uses the GitHub CLI (`gh`) to fetch activity data and Claude for intelligent summarization, highlighting what needs attention and summarizing decisions/actions required.

## Core Functionality

The tool addresses the challenge of managing high-volume GitHub activity across many projects with varying maturity levels and community engagement. Key features:

- Fetches GitHub activity (issues, PRs, comments, mentions) since the last report
- Uses Claude to intelligently summarize and highlight relevant information
- Generates markdown reports saved to a configurable directory
- Supports label-based organization with reusable watch rules
- Dynamic repository detection based on user activity
- Allows per-repo configuration of what to watch
- Smart caching to support interruption recovery and reduce API calls

## Configuration

- Configuration file: `~/.gh-report.toml`
- Default report location: `~/Github Reports`
- Default file name format: `{yyyy-mm-dd} - Github - {short-title}`
  - `short-title` is an 8-word-or-fewer AI-generated title based on report content

## Key Development Considerations

### External Dependencies
- GitHub CLI (`gh`) - Required for fetching GitHub data
  - Minimum version: 2.20.0 (check on startup)
  - Use `--json` flag for structured output
  - Use `--paginate` for automatic pagination
- Claude API - Required for summarization and intelligent processing

### GitHub Integration Decisions
- **Authentication**: Use `gh` CLI exclusively (no direct API)
- **Deleted repos**: Track and report in "Repository Changes" section
- **Comment strategy**: Cache processed context to avoid re-fetching
- **Pagination**: 100 items per page (GitHub max)
- **GitHub Enterprise**: Not supported (simplifies implementation)

### Library Choices
- **Date/Time**: Use `jiff` (v0.2) instead of `chrono` - it's more modern, safer, and has better timezone handling
- **Testing**: Use `insta` for snapshot testing - great for verifying markdown report outputs
- **HTTP Mocking**: Use `wiremock` for mocking Claude API responses in tests
- **Progress UI**: Use `indicatif` for progress bars and spinners (Phase 2+)
- **Terminal Detection**: Use `atty` to detect if stdout is a terminal
- **Caching**: Use `sha2` for content hashing and `flate2` for compression

### Report Generation Logic
- By default, generates reports covering activity since the last report was run
- CLI option available to specify a custom date range
- Reports include:
  - New repositories
  - New issues/PRs
  - Comments on issues/PRs
  - GitHub mentions

### User Context Awareness
The tool should understand user context through:
- Free-form text descriptions for each label explaining what the user cares about
- Per-repository configuration of what activity to track using watch rules
- Dynamic repository detection based on recent activity (commits > PRs > issues > comments)
- These contexts guide Claude's summarization to surface relevant information

### Watch Rules
Predefined watch rules that can be applied to labels or repos:
- `api_changes` - Public API changes, deprecations, new features
- `breaking_changes` - Breaking changes, migrations, major versions
- `security_issues` - Security vulnerabilities, CVEs, exploits
- `performance` - Performance regressions, benchmarks
- `mentions` - Direct @mentions of the user
- `review_requests` - PRs requesting review
- `all_activity` - Watch everything

## CLI Commands

- `gh-report` - Generate a report (main command)
  - `--config <path>` - Override config file location
  - `--since <date>` - Override automatic date detection
  - `--output <path>` - Override output file location
  - `--dry-run` - Preview what would be fetched
  - `--estimate-cost` - Show estimated Claude API cost
  - `--no-cache` - Bypass cache for fresh data
  - `--clear-cache` - Clear all cached data before running
- `gh-report init` - Analyze GitHub activity and generate initial config
- `gh-report rebuild-state` - Rebuild state from existing reports

## Architecture Notes

### Development Phases
1. **Phase 1**: Core functionality (synchronous, basic CLI)
2. **Phase 2**: Claude integration and intelligent summarization
3. **Phase 3**: Dynamic repos and label system
4. **Phase 4**: Polish (async/concurrent, progress UI, caching)

### Key Design Decisions
1. Clean separation between GitHub data fetching, Claude processing, and report generation
2. Robust configuration management with sensible defaults
3. Error handling for API failures (GitHub and Claude)
4. Start synchronous, add concurrency in Phase 4
5. Cache API responses in `~/Github Reports/.cache/` for interruption recovery

### Claude API Integration
- Support model aliases ("sonnet", "haiku") that auto-resolve to latest versions
- Authentication via environment variable (`ANTHROPIC_API_KEY`) preferred
- Optional API key helper scripts for dynamic credentials
- Different models for different importance levels (cost optimization)

## Development Workflow

### IMPORTANT: After Completing Any Task
1. **Update IMPLEMENTATION.md** - Mark completed items, add notes about what was delivered
2. **Update CLAUDE.md** - Add implementation details, API notes, gotchas discovered
3. **Test the changes** - Run `cargo test` and manual testing
4. **Then commit** - Only commit after documentation is updated

This ensures continuity between sessions and helps future instances understand the codebase state.

## Testing Approach

The project uses a multi-layered testing strategy:

1. **Enum-based mocking** - Use enum variants for test implementations rather than traits
2. **Snapshot testing with `insta`** - Capture and verify report outputs
3. **HTTP mocking with `wiremock`** - Mock Claude API responses without hitting real endpoints
4. **Fixture-based testing** - Store sample GitHub API responses as JSON fixtures

Key testing commands:
- `cargo test` - Run all tests
- `cargo insta review` - Review snapshot changes
- `cargo test --no-default-features` - Test without optional features

## Rust Design Patterns

### Enum-Based Abstraction
When abstracting implementations (e.g., for testing), use enums instead of traits:

```rust
// CORRECT: Enum-based abstraction
pub enum Claude {
    Real(RealClient),
    #[cfg(test)]
    Mock(MockClient),
}

// AVOID: Trait-based abstraction for simple cases
trait ClaudeClient {
    async fn complete(&self, request: Request) -> Result<Response>;
}
```

**Why**: This pattern is more idiomatic in Rust because it:
- Avoids unnecessary dynamic dispatch
- Makes all implementations explicit and discoverable
- Enables better compiler optimizations
- Simplifies the type system (no trait objects or generics needed)
- Makes testing cleaner with conditional compilation

Use this pattern for:
- API clients (Claude, GitHub)
- Storage backends
- Configuration sources
- Any abstraction with a known, finite set of implementations

## Current Implementation Status

### Completed
**Milestone 1: Foundation**
- ✅ CLI structure with clap (src/cli.rs)
- ✅ Configuration management (src/config.rs)
  - TOML parsing with serde
  - Default values for all fields
  - Home directory expansion
- ✅ State management (src/state.rs)
  - JSON persistence
  - Activity tracking
  - Auto-cleanup of inactive repos
- ✅ Main entry point with command dispatch
- ✅ Error handling with anyhow
- ✅ Logging with tracing

**Milestone 2: GitHub Integration**
- ✅ GitHub module (src/github/)
  - GitHubClient enum with Real/Mock variants
  - gh CLI subprocess execution
  - Version checking (min 2.20.0)
  - Complete data models for Issues, PRs, Comments
  - Date-based filtering support
  - Mock implementation for testing

**Milestone 3: Report Generation v1**
- ✅ Report module (src/report/)
  - Report struct for holding generated reports
  - ReportGenerator with GitHub client integration
  - ReportTemplate for markdown formatting
  - Activity grouping by repository
  - File naming with date placeholders
  - Automatic report directory creation
  - Integration with main command
- ✅ Basic markdown generation without AI
  - Header with date range
  - Summary statistics
  - Activity grouped by repository
  - Issue/PR categorization (new vs updated)
  - Clickable links to GitHub items

**Milestone 4: Claude Integration**
- ✅ Claude module (src/claude/)
  - ClaudeClient enum with Real/Mock variants
  - Complete Messages API implementation
  - Model alias resolution (sonnet → claude-3-5-sonnet-20241022)
  - Cost estimation based on token usage
  - API key from ANTHROPIC_API_KEY env var
  - Retry logic with exponential backoff
- ✅ Prompt engineering
  - System prompt for GitHub summarization
  - Activity summarization prompts
  - Title generation from summaries
  - Context-aware prompting support
- ✅ Report generator integration
  - Optional AI summaries when API key available
  - Graceful fallback without Claude
  - Cost tracking in reports

### API Notes
- **jiff date/time**: Use hours for Timestamp arithmetic, not days
  - Example: `(days as i64 * 24).hours()` instead of `days.days()`
- **Configuration**: Always expand tilde paths with `dirs::home_dir()`
- **State**: Track repos with activity scores and auto-removal
- **Context caching**: Store AI-generated summaries of issues/PRs to avoid re-processing
  - Cache location: `~/Github Reports/.cache/contexts/`
  - Include: summary, key points, last processed comment ID
  - Enables incremental processing of long discussions

## Common Development Tasks

### Running the tool
```bash
cargo run -- --dry-run                    # Preview what would be fetched
cargo run                                  # Generate report (currently stub)
cargo run -- --no-cache                   # Force fresh data
cargo run -- init                         # Initialize configuration
cargo run -- --help                       # Show all options
```

### Testing
```bash
cargo test                                 # Run all tests
cargo test state::                        # Run state module tests
cargo insta review                        # Review snapshot changes (when added)
```

### Linting and Type Checking
```bash
cargo clippy -- -D warnings              # Lint with warnings as errors
cargo fmt                                 # Auto-format code
cargo build                               # Type check

## Implementation Tips

1. **Start Simple**: Begin with synchronous code in Phase 1, add async complexity later
2. **Test with Fixtures**: Store sample GitHub API responses as JSON for consistent testing
3. **Use Snapshots**: Leverage `insta` for testing report generation
4. **Model Resolution**: Remember to resolve model aliases ("sonnet" → "claude-3-5-sonnet-20241022")
5. **Cache Keys**: Use SHA256 hash of request content for Claude cache keys
6. **Progress UI**: Only show progress indicators when stdout is a terminal (check with `atty`)