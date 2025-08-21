use anyhow::{anyhow, Result};

/// Represents a parsed GitHub issue or PR reference
#[derive(Debug, Clone, PartialEq)]
pub struct IssueReference {
    /// Repository owner (e.g., "tokio-rs")
    pub owner: String,
    /// Repository name (e.g., "tokio")
    pub repo: String,
    /// Issue or PR number
    pub number: u32,
    /// Whether this is a pull request (vs issue)
    pub is_pull_request: Option<bool>, // None means unknown from reference
}

impl IssueReference {
    /// Get the full repository name (owner/repo)
    pub fn repo_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Get a short display representation
    pub fn display(&self) -> String {
        format!("{}#{}", self.repo_name(), self.number)
    }

    /// Get the GitHub URL for this issue/PR
    pub fn url(&self) -> String {
        let item_type = match self.is_pull_request {
            Some(true) => "pull",
            Some(false) => "issues",
            None => "issues", // Default to issues if unknown
        };
        format!(
            "https://github.com/{}/{}/{}",
            self.repo_name(),
            item_type,
            self.number
        )
    }
}

/// Parse various formats of GitHub issue/PR references
pub fn parse_issue_reference(input: &str) -> Result<IssueReference> {
    let input = input.trim();

    // Try parsing as full URL first
    if let Ok(reference) = parse_github_url(input) {
        return Ok(reference);
    }

    // Try parsing as shorthand (owner/repo#123)
    if let Ok(reference) = parse_shorthand_reference(input) {
        return Ok(reference);
    }

    Err(anyhow!("Invalid issue reference format. Expected URL (https://github.com/owner/repo/issues/123) or shorthand (owner/repo#123)"))
}

/// Parse a full GitHub URL
fn parse_github_url(url: &str) -> Result<IssueReference> {
    if !url.starts_with("https://github.com/") {
        return Err(anyhow!("Not a GitHub URL"));
    }

    let path = url.strip_prefix("https://github.com/").unwrap();
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() < 4 {
        return Err(anyhow!("URL too short"));
    }

    let owner = parts[0].to_string();
    let repo = parts[1].to_string();
    let item_type = parts[2];
    let number_str = parts[3];

    // Determine if it's a PR or issue
    let is_pull_request = match item_type {
        "pull" => Some(true),
        "issues" => Some(false),
        _ => return Err(anyhow!("Unknown item type: {}", item_type)),
    };

    // Parse the number
    let number = number_str
        .parse::<u32>()
        .map_err(|_| anyhow!("Invalid issue number: {}", number_str))?;

    Ok(IssueReference {
        owner,
        repo,
        number,
        is_pull_request,
    })
}

/// Parse shorthand format: owner/repo#123
fn parse_shorthand_reference(input: &str) -> Result<IssueReference> {
    if !input.contains('#') {
        return Err(anyhow!("Missing # separator"));
    }

    let parts: Vec<&str> = input.split('#').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid format"));
    }

    let repo_part = parts[0];
    let number_str = parts[1];

    // Parse repo part (owner/repo)
    if !repo_part.contains('/') {
        return Err(anyhow!("Missing / separator in repository"));
    }

    let repo_parts: Vec<&str> = repo_part.split('/').collect();
    if repo_parts.len() != 2 {
        return Err(anyhow!("Invalid repository format"));
    }

    let owner = repo_parts[0].to_string();
    let repo = repo_parts[1].to_string();

    // Parse the number
    let number = number_str
        .parse::<u32>()
        .map_err(|_| anyhow!("Invalid issue number: {}", number_str))?;

    // We don't know if it's a PR or issue from shorthand format
    Ok(IssueReference {
        owner,
        repo,
        number,
        is_pull_request: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_issue_url() {
        let url = "https://github.com/tokio-rs/tokio/issues/7546";
        let reference = parse_issue_reference(url).unwrap();

        assert_eq!(reference.owner, "tokio-rs");
        assert_eq!(reference.repo, "tokio");
        assert_eq!(reference.number, 7546);
        assert_eq!(reference.is_pull_request, Some(false));
        assert_eq!(reference.repo_name(), "tokio-rs/tokio");
        assert_eq!(reference.display(), "tokio-rs/tokio#7546");
    }

    #[test]
    fn test_parse_github_pr_url() {
        let url = "https://github.com/rust-lang/rust/pull/123456";
        let reference = parse_issue_reference(url).unwrap();

        assert_eq!(reference.owner, "rust-lang");
        assert_eq!(reference.repo, "rust");
        assert_eq!(reference.number, 123456);
        assert_eq!(reference.is_pull_request, Some(true));
        assert_eq!(
            reference.url(),
            "https://github.com/rust-lang/rust/pull/123456"
        );
    }

    #[test]
    fn test_parse_shorthand_reference() {
        let shorthand = "tokio-rs/tokio#7546";
        let reference = parse_issue_reference(shorthand).unwrap();

        assert_eq!(reference.owner, "tokio-rs");
        assert_eq!(reference.repo, "tokio");
        assert_eq!(reference.number, 7546);
        assert_eq!(reference.is_pull_request, None);
        assert_eq!(reference.display(), "tokio-rs/tokio#7546");
    }

    #[test]
    fn test_parse_various_shorthand_formats() {
        // Test with different repo names
        let reference = parse_issue_reference("microsoft/TypeScript#123").unwrap();
        assert_eq!(reference.owner, "microsoft");
        assert_eq!(reference.repo, "TypeScript");
        assert_eq!(reference.number, 123);

        // Test with numeric repo
        let reference = parse_issue_reference("user/repo123#456").unwrap();
        assert_eq!(reference.owner, "user");
        assert_eq!(reference.repo, "repo123");
        assert_eq!(reference.number, 456);
    }

    #[test]
    fn test_parse_invalid_formats() {
        // Missing #
        assert!(parse_issue_reference("tokio-rs/tokio").is_err());

        // Missing /
        assert!(parse_issue_reference("tokio#123").is_err());

        // Invalid number
        assert!(parse_issue_reference("tokio-rs/tokio#abc").is_err());

        // Not a GitHub URL
        assert!(parse_issue_reference("https://gitlab.com/owner/repo/issues/123").is_err());

        // Empty string
        assert!(parse_issue_reference("").is_err());

        // Just a number
        assert!(parse_issue_reference("123").is_err());
    }

    #[test]
    fn test_reference_url_generation() {
        // Issue reference (unknown type)
        let reference = IssueReference {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 123,
            is_pull_request: None,
        };
        assert_eq!(reference.url(), "https://github.com/owner/repo/issues/123");

        // Explicit issue
        let reference = IssueReference {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 123,
            is_pull_request: Some(false),
        };
        assert_eq!(reference.url(), "https://github.com/owner/repo/issues/123");

        // Explicit PR
        let reference = IssueReference {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 123,
            is_pull_request: Some(true),
        };
        assert_eq!(reference.url(), "https://github.com/owner/repo/pull/123");
    }
}
