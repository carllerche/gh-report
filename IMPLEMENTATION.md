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

### Milestone 2: GitHub Data Collection (2-3 days)
**Goal**: Fetch GitHub data via gh CLI

**Deliverables**:
- [ ] GitHub client wrapper around `gh` CLI
- [ ] Data models for issues, PRs, comments
- [ ] Basic filtering by date range
- [ ] Repository list management

**Testing Strategy**:
- Mock `gh` CLI responses with fixture files
- Unit tests for data parsing
- Integration tests with mock subprocess

**Design Document Required**: [GitHub Integration Strategy](docs/design/01-github-integration.md)
- How to efficiently call `gh` CLI
- Error handling for gh failures
- Data transformation approach

### Milestone 3: Report Generation v1 (2 days)
**Goal**: Generate basic markdown reports without AI

**Deliverables**:
- [ ] Markdown report generator
- [ ] Basic template system
- [ ] Report file naming and storage
- [ ] Simple activity grouping (by repo)

**Testing Strategy**:
- Snapshot tests for generated reports
- Tests for different data scenarios (empty, large, etc.)
- File system operation tests

### Milestone 4: Claude Integration (3-4 days)
**Goal**: Add AI-powered summarization

**Deliverables**:
- [ ] Claude API client implementation
- [ ] Request/response models
- [ ] Model alias resolution
- [ ] Cost estimation
- [ ] Basic prompt templates

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

### Milestone 5: Intelligent Summarization (3-4 days)
**Goal**: Implement smart filtering and summarization

**Deliverables**:
- [ ] Watch rules engine
- [ ] Label system implementation
- [ ] Priority scoring algorithm
- [ ] Context-aware summarization
- [ ] Action suggestion generation

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

### Milestone 6: Dynamic Repository Management (2-3 days)
**Goal**: Auto-detect and manage repositories

**Deliverables**:
- [ ] Activity scoring system
- [ ] Auto-add/remove logic
- [ ] `gh-report init` command
- [ ] Repository activity analysis

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