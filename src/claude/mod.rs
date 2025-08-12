use anyhow::{Context, Result};
use std::env;

mod models;
mod client;
mod cli_client;
mod claude_interface;
pub mod prompts;

pub use models::*;
pub use client::*;
pub use cli_client::ClaudeCLI;
pub use claude_interface::ClaudeInterface;

#[cfg(test)]
pub use client::MockClaude;

/// Resolve model alias to full model name
pub fn resolve_model_alias(alias: &str) -> String {
    match alias.to_lowercase().as_str() {
        "sonnet" | "sonnet-3.5" => "claude-3-5-sonnet-20241022".to_string(),
        "haiku" | "haiku-3.5" => "claude-3-5-haiku-20241022".to_string(),
        "opus" | "opus-3" => "claude-3-opus-20240229".to_string(),
        _ => alias.to_string(), // Return as-is if not an alias
    }
}

/// Get API key from environment
pub fn get_api_key() -> Result<String> {
    env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable not set")
}

/// Estimate cost for a request in dollars
pub fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f32 {
    // Pricing as of late 2024 (per 1M tokens)
    let (input_price, output_price) = match model {
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("haiku") => (0.25, 1.25),
        m if m.contains("opus") => (15.0, 75.0),
        _ => (3.0, 15.0), // Default to Sonnet pricing
    };
    
    let input_cost = (input_tokens as f32 / 1_000_000.0) * input_price;
    let output_cost = (output_tokens as f32 / 1_000_000.0) * output_price;
    
    input_cost + output_cost
}

/// Estimate token count for text (rough approximation)
pub fn estimate_tokens(text: &str) -> u32 {
    // Rough estimate: ~4 characters per token for English text
    (text.len() as f32 / 4.0).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resolve_model_alias() {
        assert_eq!(resolve_model_alias("sonnet"), "claude-3-5-sonnet-20241022");
        assert_eq!(resolve_model_alias("haiku"), "claude-3-5-haiku-20241022");
        assert_eq!(resolve_model_alias("opus"), "claude-3-opus-20240229");
        assert_eq!(resolve_model_alias("claude-3-custom"), "claude-3-custom");
    }
    
    #[test]
    fn test_estimate_cost() {
        // Test Sonnet pricing
        let cost = estimate_cost("claude-3-5-sonnet-20241022", 1000, 500);
        assert!((cost - 0.0105).abs() < 0.0001);
        
        // Test Haiku pricing
        let cost = estimate_cost("claude-3-5-haiku-20241022", 1000, 500);
        assert!((cost - 0.00088).abs() < 0.00001);
    }
    
    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("Hello world"), 3);
        assert_eq!(estimate_tokens("This is a longer sentence with more tokens"), 11);
    }
}