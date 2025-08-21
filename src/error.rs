use std::fmt;

/// User-friendly error wrapper
#[derive(Debug)]
pub struct UserError {
    message: String,
    details: Option<String>,
    suggestion: Option<String>,
}

impl UserError {
    /// Create a new user error
    pub fn new(message: impl Into<String>) -> Self {
        UserError {
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    /// Add details about the error
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Add a suggestion for how to fix the error
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Get the error message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Format the error for display
    pub fn display(&self) {
        eprintln!("\n❌ Error: {}", self.message);

        if let Some(ref details) = self.details {
            eprintln!("\n   {}", details);
        }

        if let Some(ref suggestion) = self.suggestion {
            eprintln!("\n💡 {}", suggestion);
        }
    }
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref details) = self.details {
            write!(f, ": {}", details)?;
        }
        Ok(())
    }
}

impl std::error::Error for UserError {}

/// Convert common errors to user-friendly messages
pub fn user_friendly_error(error: &anyhow::Error) -> UserError {
    let error_str = error.to_string();

    // GitHub CLI errors
    if error_str.contains("gh: command not found") || error_str.contains("GitHub CLI not found") {
        return UserError::new("GitHub CLI is not installed")
            .with_details("The 'gh' command is required to fetch GitHub data")
            .with_suggestion("Install GitHub CLI from https://cli.github.com/");
    }

    if error_str.contains("gh auth login") || error_str.contains("not authenticated") {
        return UserError::new("Not authenticated with GitHub")
            .with_details("You need to log in to GitHub CLI first")
            .with_suggestion("Run 'gh auth login' to authenticate");
    }

    // Claude API errors
    if error_str.contains("ANTHROPIC_API_KEY") {
        return UserError::new("Anthropic API key not configured")
            .with_details("Claude AI features require an API key")
            .with_suggestion("Set the ANTHROPIC_API_KEY environment variable");
    }

    if error_str.contains("401") && error_str.contains("anthropic") {
        return UserError::new("Invalid Anthropic API key")
            .with_details("The provided API key was rejected by Claude's API")
            .with_suggestion("Check your ANTHROPIC_API_KEY is correct");
    }

    if error_str.contains("rate limit") {
        return UserError::new("API rate limit exceeded")
            .with_details("Too many requests have been made recently")
            .with_suggestion("Wait a few minutes and try again");
    }

    // Configuration errors
    if error_str.contains("Failed to read config") {
        return UserError::new("Configuration file not found")
            .with_details("No .gh-report.toml file found")
            .with_suggestion("Run 'gh-report init' to create a configuration");
    }

    if error_str.contains("Failed to parse config") {
        return UserError::new("Invalid configuration file")
            .with_details("The configuration file contains syntax errors")
            .with_suggestion("Check the TOML syntax in your .gh-report.toml file");
    }

    // Permission errors
    if error_str.contains("Permission denied") {
        return UserError::new("Permission denied")
            .with_details("Cannot write to the specified location")
            .with_suggestion("Check that you have write permissions to the report directory");
    }

    // Network errors
    if error_str.contains("network") || error_str.contains("connection") {
        return UserError::new("Network connection failed")
            .with_details("Could not connect to GitHub or Claude APIs")
            .with_suggestion("Check your internet connection and try again");
    }

    // Default fallback
    UserError::new("An unexpected error occurred").with_details(error_str)
}

/// Wrap a result with user-friendly error handling
pub trait UserFriendly<T> {
    fn user_friendly(self) -> Result<T, UserError>;
}

impl<T> UserFriendly<T> for anyhow::Result<T> {
    fn user_friendly(self) -> Result<T, UserError> {
        self.map_err(|e| user_friendly_error(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn test_user_error_creation() {
        let error = UserError::new("Test error")
            .with_details("Some details")
            .with_suggestion("Try this");

        assert_eq!(error.message, "Test error");
        assert_eq!(error.details, Some("Some details".to_string()));
        assert_eq!(error.suggestion, Some("Try this".to_string()));
    }

    #[test]
    fn test_user_error_display() {
        let error = UserError::new("Test error").with_details("Some details");

        let display = format!("{}", error);
        assert_eq!(display, "Test error: Some details");

        let error = UserError::new("Test error");
        let display = format!("{}", error);
        assert_eq!(display, "Test error");
    }

    #[test]
    fn test_github_cli_error_detection() {
        let error = anyhow!("gh: command not found");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "GitHub CLI is not installed");
        assert!(user_error.suggestion.is_some());
    }

    #[test]
    fn test_auth_error_detection() {
        let error = anyhow!("gh auth login required");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "Not authenticated with GitHub");
        assert!(user_error.suggestion.unwrap().contains("gh auth login"));
    }

    #[test]
    fn test_api_key_error_detection() {
        let error = anyhow!("ANTHROPIC_API_KEY not set");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "Anthropic API key not configured");
        assert!(user_error.suggestion.unwrap().contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn test_rate_limit_error_detection() {
        let error = anyhow!("rate limit exceeded");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "API rate limit exceeded");
        assert!(user_error.suggestion.unwrap().contains("Wait"));
    }

    #[test]
    fn test_config_error_detection() {
        let error = anyhow!("Failed to read config");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "Configuration file not found");
        assert!(user_error.suggestion.unwrap().contains("gh-report init"));
    }

    #[test]
    fn test_permission_error_detection() {
        let error = anyhow!("Permission denied");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "Permission denied");
        assert!(user_error.details.unwrap().contains("Cannot write"));
    }

    #[test]
    fn test_network_error_detection() {
        let error = anyhow!("network connection failed");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "Network connection failed");
        assert!(user_error.suggestion.unwrap().contains("internet"));
    }

    #[test]
    fn test_unknown_error_fallback() {
        let error = anyhow!("Some random error");
        let user_error = user_friendly_error(&error);

        assert_eq!(user_error.message, "An unexpected error occurred");
        assert_eq!(user_error.details, Some("Some random error".to_string()));
    }
}
