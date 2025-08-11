# Implementation Plan

This document outlines the implementation roadmap for gh-daily-report, breaking the work into manageable milestones with clear deliverables and testing strategies.

## Project Structure

```
gh-daily-report/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── cli.rs               # Command-line argument parsing
│   ├── config.rs            # Configuration management
│   ├── github/              # GitHub interaction module
│   ├── claude/              # Claude API client
│   ├── report/              # Report generation
│   ├── cache/               # Caching layer
│   └── state.rs             # State management
├── tests/                   # Integration tests
├── fixtures/                # Test fixtures (JSON samples)
├── docs/                    # Detailed documentation
│   ├── design/              # Design documents
│   └── examples/            # Example configs and reports
└── snapshots/               # Insta snapshot tests
```

## Implementation Milestones

### Milestone 1: Foundation ✅ COMPLETE
**Goal**: Basic CLI structure with configuration management

**Deliverables**:
- [x] Project setup with dependencies
- [x] CLI argument parsing with clap
- [x] Configuration file parsing (TOML)
- [x] State file management (JSON)
- [x] Basic error handling structure

**Completed**: 2024-01-11
- All CLI commands implemented (main, init, rebuild-state)
- Full configuration system with defaults and validation
- State persistence with automatic cleanup of inactive repos
- Comprehensive error handling with anyhow
- 3 passing tests for state management

**Testing Strategy**:
- Unit tests for config parsing and validation
- Snapshot tests for example configurations
- Tests for state file read/write operations

**Implementation Notes**:
- Start with synchronous code only
- Focus on robust error messages
- Implement config validation thoroughly

### Milestone 2: GitHub Data Collection ✅ COMPLETE
**Goal**: Fetch GitHub data via gh CLI

**Deliverables**:
- [x] GitHub client wrapper around `gh` CLI
- [x] Data models for issues, PRs, comments
- [x] Basic filtering by date range
- [x] Repository list management

**Completed**: 2024-01-11
- Full GitHubClient with Real/Mock enum pattern
- gh version checking (min 2.20.0)
- Complete data models with serde
- Mock implementation for testing
- 8 passing tests including fixtures

**Testing Strategy**:
- Mock `gh` CLI responses with fixture files
- Unit tests for data parsing
- Integration tests with mock subprocess

**Design Document Required**: [GitHub Integration Strategy](docs/design/01-github-integration.md)
- How to efficiently call `gh` CLI
- Error handling for gh failures
- Data transformation approach

### Milestone 3: Report Generation v1 ✅ COMPLETE
**Goal**: Generate basic markdown reports without AI

**Deliverables**:
- [x] Markdown report generator
- [x] Basic template system
- [x] Report file naming and storage
- [x] Simple activity grouping (by repo)

**Completed**: 2024-01-11
- Full markdown report generation with templates
- Customizable file naming with date placeholders
- Activity grouping by repository
- Template rendering with headers, summary, and footer
- 5 passing tests for report generation

**Testing Strategy**:
- Snapshot tests for generated reports
- Tests for different data scenarios (empty, large, etc.)
- File system operation tests

### Milestone 4: Claude Integration ✅ COMPLETE
**Goal**: Add AI-powered summarization

**Deliverables**:
- [x] Claude API client implementation
- [x] Request/response models
- [x] Model alias resolution
- [x] Cost estimation
- [x] Basic prompt templates

**Completed**: 2024-01-11
- Full Claude API client with enum pattern (Real/Mock)
- Complete Messages API models with builder pattern
- Model alias resolution (sonnet, haiku, opus)
- Cost estimation based on token usage
- Prompt templates for summarization and title generation
- Integration with report generator (optional AI summaries)
- 10 passing tests for Claude functionality

**Testing Strategy**:
- Mock HTTP responses with wiremock
- Unit tests for model resolution
- Integration tests for API client
- Cost calculation tests

**Design Document Required**: [Claude Integration Architecture](docs/design/02-claude-integration.md)
- Prompt engineering strategy
- Batching and prioritization logic
- Error recovery approach
- Cost optimization strategies

### Milestone 5: Intelligent Summarization ✅ COMPLETE
**Goal**: Implement smart filtering and summarization

**Deliverables**:
- [x] Watch rules engine
- [x] Label system implementation
- [x] Priority scoring algorithm
- [x] Context-aware summarization
- [x] Action suggestion generation

**Completed**: 2024-01-11
- Full intelligence module with analyzer, scoring, and context
- Watch rules engine with pattern matching
- Priority scoring based on importance, recency, activity, and rules
- Action item extraction with urgency levels
- Context injection for AI summarization
- Integration with report generator
- 10 passing tests for intelligence features

**Testing Strategy**:
- Unit tests for scoring algorithms
- Fixture-based tests for watch rules
- Snapshot tests for summaries
- Mock Claude responses for different scenarios

**Design Document Required**: [Summarization Logic](docs/design/03-summarization-logic.md)
- Scoring algorithm details
- Watch rule matching implementation
- Context injection for Claude
- Handling edge cases (empty data, API limits)

### Milestone 6: Dynamic Repository Management ✅ COMPLETE
**Goal**: Auto-detect and manage repositories

**Deliverables**:
- [x] Activity scoring system
- [x] Auto-add/remove logic
- [x] `gh-report init` command
- [x] Repository activity analysis

**Completed**: 2024-01-11
- Dynamic repository discovery via gh search
- Activity scoring based on commits/PRs/issues/comments
- Auto-add repositories with high activity
- Auto-remove inactive repositories
- gh-report init command with repository discovery
- Integration with main report generation
- 6 passing tests for dynamic management

**Testing Strategy**:
- Unit tests for scoring calculations
- Integration tests for init command
- Tests for add/remove thresholds

### Milestone 7: Caching Layer (2-3 days)
**Goal**: Implement caching for interruption recovery

**Deliverables**:
- [ ] Cache directory structure
- [ ] GitHub response caching
- [ ] Claude response caching
- [ ] Cache invalidation logic
- [ ] Compression implementation

**Testing Strategy**:
- Unit tests for cache operations
- Tests for cache expiry
- Integration tests for interruption recovery
- Performance tests for compression

**Design Document Required**: [Caching Strategy](docs/design/04-caching-strategy.md)
- Cache key generation
- Invalidation rules
- Compression trade-offs
- Disk space management

### Milestone 8: User Experience Polish (2-3 days)
**Goal**: Add progress indicators and better UX

**Deliverables**:
- [ ] Progress bars with indicatif
- [ ] Dry-run implementation
- [ ] Better error messages
- [ ] Terminal detection (atty)
- [ ] Interrupt handling (Ctrl-C)

**Testing Strategy**:
- Manual testing for UI elements
- Tests for terminal detection
- Integration tests for dry-run
- Signal handling tests

### Milestone 9: Performance & Concurrency (3-4 days)
**Goal**: Optimize with async/concurrent operations

**Deliverables**:
- [ ] Async/await migration
- [ ] Concurrent GitHub fetching
- [ ] Concurrent Claude requests
- [ ] Rate limiting implementation
- [ ] Performance benchmarks

**Testing Strategy**:
- Concurrent operation tests
- Rate limit tests
- Performance benchmarks
- Load tests with many repos

**Design Document Required**: [Concurrency Architecture](docs/design/05-concurrency.md)
- Semaphore strategies
- Error aggregation in concurrent ops
- Progress reporting with concurrency
- Memory management

### Milestone 10: Final Polish & Documentation (2 days)
**Goal**: Production readiness

**Deliverables**:
- [ ] Comprehensive documentation
- [ ] Example configurations
- [ ] Installation guide
- [ ] Performance tuning
- [ ] Release preparation

**Testing Strategy**:
- End-to-end tests
- Documentation verification
- Cross-platform testing

## Total Timeline

Estimated: 25-35 days of focused development

## Critical Path

The following items block other work:
1. Milestone 1 (Foundation) - blocks everything
2. Milestone 2 (GitHub) - blocks 3, 5, 6
3. Milestone 4 (Claude) - blocks 5
4. Milestone 5 (Summarization) - blocks meaningful testing of later features

## Risk Areas

1. **GitHub CLI stability** - Dependency on external tool
2. **Claude API changes** - API is still evolving
3. **Performance with many repos** - May need optimization earlier
4. **Prompt engineering** - May require iteration to get right

## Success Criteria

Each milestone must:
1. Pass all tests (unit, integration, snapshot)
2. Have documentation updated
3. Work with example configurations
4. Handle errors gracefully
5. Maintain backward compatibility (after v1.0)

## Next Steps

1. Create `docs/design/` directory structure
2. Write design document for GitHub integration
3. Set up initial project with Milestone 1 dependencies
4. Begin implementation of foundation components

## Design Documents Required

The following components need design review before implementation:

1. [GitHub Integration Strategy](docs/design/01-github-integration.md) - Milestone 2
2. [Claude Integration Architecture](docs/design/02-claude-integration.md) - Milestone 4
3. [Summarization Logic](docs/design/03-summarization-logic.md) - Milestone 5
4. [Caching Strategy](docs/design/04-caching-strategy.md) - Milestone 7
5. [Concurrency Architecture](docs/design/05-concurrency.md) - Milestone 9

Each design document should include:
- Problem statement
- Proposed solution
- Alternative approaches considered
- Implementation details
- Testing approach
- Risk mitigation