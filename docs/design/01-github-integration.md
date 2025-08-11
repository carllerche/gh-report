# GitHub Integration Strategy

## Problem Statement

We need to fetch various types of GitHub data (issues, PRs, comments, mentions) efficiently using the `gh` CLI tool. This involves:
- Managing subprocess calls to `gh`
- Parsing JSON output from various endpoints
- Handling errors and rate limits
- Transforming data into our domain models
- Dealing with pagination for large result sets

## Requirements

### Functional Requirements
- Fetch issues/PRs for multiple repositories
- Get comments on specific issues/PRs
- Search for user mentions across GitHub
- Handle authentication via existing `gh` auth
- Support date-based filtering
- Handle private repositories

### Performance Requirements
- Minimize number of `gh` calls
- Support future concurrent fetching (Milestone 9)
- Cache-friendly response handling
- Handle up to 100 repositories efficiently

### Constraints
- Must use `gh` CLI (not direct API)
- Cannot modify `gh` configuration
- Must handle `gh` not being installed gracefully

## Proposed Solution

### Architecture

```rust
// src/github/mod.rs
pub enum GitHubClient {
    Real(RealGitHub),
    #[cfg(test)]
    Mock(MockGitHub),
}

pub struct RealGitHub {
    gh_path: PathBuf,  // Path to gh executable
}

// Data models
pub struct Issue {
    pub number: u32,
    pub title: String,
    pub body: String,
    pub author: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub state: IssueState,
    pub labels: Vec<String>,
}
```

### Subprocess Management

Use `std::process::Command` for now (sync), preparing for tokio::process later:

```rust
impl RealGitHub {
    fn execute_gh(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.gh_path)
            .args(args)
            .output()
            .context("Failed to execute gh")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("gh command failed: {}", stderr));
        }
        
        Ok(String::from_utf8(output.stdout)?)
    }
}
```

### Data Fetching Strategy

1. **Batch by repository** - One call per repo for issues/PRs
2. **Use search API for mentions** - More efficient than checking each repo
3. **Leverage `gh` JSON output** - Use `--json` flag for structured data
4. **Handle pagination** - Use `--limit` and `--paginate` flags

### Example Commands

```bash
# Get issues and PRs for a repo
gh api repos/{owner}/{repo}/issues --json number,title,body,author,createdAt,updatedAt,state,labels

# Search for mentions
gh api search/issues --params "involves:@me updated:>2024-01-01"

# Get comments on an issue
gh api repos/{owner}/{repo}/issues/{number}/comments --json body,author,createdAt

# Check gh version
gh version
```

### Context Caching Strategy

To avoid re-processing entire issue/PR histories on each run, we'll cache processed context:

```rust
pub struct IssueContext {
    pub issue_number: u32,
    pub repo: String,
    pub last_updated: Timestamp,
    pub summary: String,  // AI-generated summary of discussion so far
    pub key_points: Vec<String>,  // Important decisions/points
    pub participants: Vec<String>,
    pub last_processed_comment_id: Option<u64>,
}
```

**Cache workflow**:
1. Check if context exists for issue/PR
2. If yes, fetch only new comments since `last_processed_comment_id`
3. If new comments reference earlier discussion not in context:
   - Fetch more historical comments
   - Regenerate context with fuller picture
4. Update cached context with new information
5. Use context + new comments for report generation

**Benefits**:
- Reduces API calls for old comments
- Preserves discussion context across runs
- Enables better AI summaries with historical awareness
- Allows incremental processing

## Alternative Approaches

### Direct GitHub API
- **Pros**: More control, better error handling, no subprocess overhead
- **Cons**: Need to handle auth ourselves, more code, duplicate what `gh` does
- **Decision**: Stay with `gh` for simplicity and auth handling

### Octocrab/Other Rust GitHub Libraries
- **Pros**: Type-safe, async native, good abstractions
- **Cons**: Another dependency, need to handle auth, may not support all features
- **Decision**: Stick with `gh` as specified in requirements

## Implementation Plan

1. **Create data models** - Issue, PR, Comment, etc.
2. **Implement GitHubClient enum** - With Real and Mock variants
3. **Add gh execution wrapper** - With error handling
4. **Implement fetch methods** - One for each data type
5. **Add JSON parsing** - Using serde_json
6. **Create mock implementation** - For testing
7. **Add integration tests** - Using fixture files

## Testing Strategy

### Unit Tests
- Mock `Command` execution using the Mock variant
- Test JSON parsing with fixture files
- Test error handling for various gh failures

### Integration Tests
```rust
#[test]
fn test_fetch_issues() {
    let client = GitHubClient::Mock(MockGitHub::new("fixtures/issues.json"));
    let issues = client.fetch_issues("owner/repo", None).unwrap();
    assert_eq!(issues.len(), 5);
}
```

### Fixture Files
Store in `fixtures/github/`:
- `issues.json` - Sample issues response
- `prs.json` - Sample PRs response
- `comments.json` - Sample comments
- `error.json` - Error response

## Risks and Mitigations

### Risk: gh CLI not installed
**Mitigation**: Check for `gh` on startup, provide clear installation instructions

### Risk: gh authentication expires
**Mitigation**: Detect auth errors, suggest `gh auth login`

### Risk: Rate limiting
**Mitigation**: 
- Implement exponential backoff
- Show progress to user
- Cache responses aggressively

### Risk: Large result sets
**Mitigation**:
- Use pagination
- Implement data limits (max 100 issues per repo)
- Summarize in Claude rather than fetching everything

### Risk: gh output format changes
**Mitigation**:
- Pin gh version in documentation
- Use `--json` for stable output
- Add tests that verify expected format

## Design Decisions

1. **Version checking**: Yes, check `gh` version on startup
   - Define minimum supported version (e.g., 2.20.0)
   - Error clearly if version is too old
   - Store version check result to avoid repeated checks

2. **Deleted/inaccessible repos**: Add "deleted repos" note
   - When repo returns 404 or permission denied
   - Add to "Repository Changes" section of report
   - Remove from future checks automatically
   - Track in state file as "deleted" with timestamp

3. **Comment fetching strategy**: Smart context building
   - Store context in cache for each issue/PR (not just raw API responses)
   - Include summary of earlier discussion in cached context
   - Fetch recent comments first (since last run)
   - If new comments reference previous discussion and context not cached:
     - Fetch more historical comments as needed
     - Update cached context with fuller picture
   - Cache location: `~/Github Reports/.cache/contexts/`

4. **Pagination size**: 100 items per page
   - GitHub's maximum for most endpoints
   - Reduces number of API calls
   - Use `--paginate` flag for automatic handling

5. **GitHub Enterprise**: Not supported
   - Simplifies implementation
   - Focus on GitHub.com only
   - Can be added later if needed

## Next Steps

1. Review this design document
2. Create fixture files from real `gh` output
3. Implement basic GitHubClient structure
4. Add one fetch method as proof of concept
5. Iterate based on learnings