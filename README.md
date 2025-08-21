# gh-report

A command-line tool that generates intelligent daily reports of GitHub activity, providing personalized summaries and actionable insights for maintainers managing multiple repositories.

## Overview

Managing multiple open source projects on GitHub means drowning in notifications. This tool solves that by:
- Fetching all your GitHub activity since the last report
- Using Claude AI to intelligently summarize and prioritize 
- Generating a clean markdown report highlighting what needs your attention
- Learning your preferences through configurable watch rules

## Features

- **Smart Summarization**: Uses Claude to understand context and surface what matters
- **Dynamic Repository Tracking**: Automatically tracks repos you're active in
- **Flexible Watch Rules**: Configure what to monitor (API changes, security issues, mentions, etc.)
- **Interruption Recovery**: Caches API responses so you can Ctrl-C and resume
- **Cost Optimization**: Uses different Claude models based on content importance

## Installation

```bash
# Clone and build from source (requires Rust)
git clone https://github.com/yourusername/gh-report
cd gh-report
cargo install --path .
```

### Prerequisites

- [GitHub CLI](https://cli.github.com/) (`gh`) installed and authenticated
- [Anthropic API key](https://console.anthropic.com/) for Claude

## Quick Start

```bash
# Set your Anthropic API key
export ANTHROPIC_API_KEY="sk-ant-..."

# Initialize configuration based on your GitHub activity
gh-report init

# Generate your first report
gh-report
```

## Configuration

The tool uses a TOML configuration file at `~/.config/gh-report/config.toml`:

```toml
[settings]
report_dir = "~/Github Reports"
max_lookback_days = 30

[claude]
primary_model = "sonnet"  # Auto-selects latest Claude 3.5 Sonnet
secondary_model = "haiku" # For less important content

[[labels]]
name = "rust-libs"
watch_rules = ["api_changes", "breaking_changes", "security_issues"]
importance = "high"
context = "I maintain these Rust libraries and need to know about API changes"

[[repos]]
name = "tokio-rs/tokio"
labels = ["rust-libs"]
importance_override = "critical"
```

## Usage

### Generate a report
```bash
gh-report
```

### Preview what would be fetched (dry run)
```bash
gh-report --dry-run
```

### Force fresh data (bypass cache)
```bash
gh-report --no-cache
```

### Generate report for specific date range
```bash
gh-report --since 2024-01-01
```

## Watch Rules

Predefined patterns you can apply to labels or repositories:

- `api_changes` - Public API changes, deprecations, new features
- `breaking_changes` - Breaking changes, migrations, major versions  
- `security_issues` - Security vulnerabilities, CVEs, exploits
- `performance` - Performance regressions, benchmarks
- `mentions` - Direct @mentions of you
- `review_requests` - PRs requesting your review
- `all_activity` - Watch everything

## Example Report

```markdown
# GitHub Activity Report - 2024-01-15

## 🚨 Action Required

### tokio-rs/tokio #6234 - Breaking Change Proposal
**Requires your approval** - PR proposes removing deprecated runtime APIs...
**Suggested response**: Approve with migration guide requirement...

## 👀 Needs Attention

### rust-lang/rust #119876 - Performance regression
Regression in async performance affecting Tokio benchmarks...

## 📰 FYI

- tokio-rs/bytes: 3 new contributors, documentation improvements
- ...
```

## Development

See [SPEC.md](SPEC.md) for the complete specification and [CLAUDE.md](CLAUDE.md) for development guidelines.

### Building from source
```bash
cargo build --release
```

### Running tests
```bash
cargo test
cargo insta review  # Review snapshot changes
```

## License

MIT

## Contributing

Contributions welcome! Please read the [specification](SPEC.md) first to understand the architecture.