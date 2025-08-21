use super::{ClaudeCLI, ClaudeClient, MessagesRequest, MessagesResponse};
use crate::config::{ClaudeBackend, ClaudeConfig};
use anyhow::{Context, Result};
use tracing::{info, warn};

/// Unified interface for Claude (API or CLI)
pub enum ClaudeInterface {
    Api(ClaudeClient),
    Cli(ClaudeCLI),
}

impl ClaudeInterface {
    /// Create a new Claude interface based on config
    pub fn new(config: &ClaudeConfig) -> Result<Option<Self>> {
        match config.backend {
            ClaudeBackend::Api => {
                // Try to create API client
                match std::env::var("ANTHROPIC_API_KEY") {
                    Ok(_) => match ClaudeClient::new() {
                        Ok(client) => {
                            info!("Using Claude API backend");
                            Ok(Some(ClaudeInterface::Api(client)))
                        }
                        Err(e) => {
                            warn!("Failed to initialize Claude API client: {}", e);
                            Ok(None)
                        }
                    },
                    Err(_) => {
                        info!("ANTHROPIC_API_KEY not set, Claude API unavailable");
                        Ok(None)
                    }
                }
            }
            ClaudeBackend::Cli => {
                // Try to create CLI client
                if ClaudeCLI::is_available() {
                    match ClaudeCLI::new(config.primary_model.clone()) {
                        Ok(client) => {
                            info!("Using Claude CLI backend");
                            Ok(Some(ClaudeInterface::Cli(client)))
                        }
                        Err(e) => {
                            warn!("Failed to initialize Claude CLI: {}", e);
                            Ok(None)
                        }
                    }
                } else {
                    info!("Claude CLI not available");
                    Ok(None)
                }
            }
            ClaudeBackend::Auto => {
                // Try CLI first, then API
                if ClaudeCLI::is_available() {
                    match ClaudeCLI::new(config.primary_model.clone()) {
                        Ok(client) => {
                            info!("Using Claude CLI backend (auto-detected)");
                            return Ok(Some(ClaudeInterface::Cli(client)));
                        }
                        Err(e) => {
                            warn!("Failed to initialize Claude CLI, trying API: {}", e);
                        }
                    }
                }

                // Fall back to API
                match std::env::var("ANTHROPIC_API_KEY") {
                    Ok(_) => match ClaudeClient::new() {
                        Ok(client) => {
                            info!("Using Claude API backend (fallback)");
                            Ok(Some(ClaudeInterface::Api(client)))
                        }
                        Err(e) => {
                            warn!("Failed to initialize Claude API client: {}", e);
                            Ok(None)
                        }
                    },
                    Err(_) => {
                        info!("No Claude backend available (CLI not installed, API key not set)");
                        Ok(None)
                    }
                }
            }
        }
    }

    /// Send a messages request
    pub fn messages(&self, request: MessagesRequest) -> Result<MessagesResponse> {
        match self {
            ClaudeInterface::Api(client) => client.messages(request),
            ClaudeInterface::Cli(client) => {
                // Convert MessagesRequest to CLI format
                let prompt = request
                    .messages
                    .iter()
                    .map(|m| m.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n\n");

                let system = request.system.as_deref();

                // Send to CLI
                let response_text = client.send_message(&prompt, system)?;

                // Convert response to MessagesResponse format
                Ok(MessagesResponse {
                    id: "cli_response".to_string(),
                    content: vec![crate::claude::Content::Text {
                        text: response_text,
                    }],
                    model: request.model,
                    stop_reason: Some("end_turn".to_string()),
                    usage: crate::claude::Usage {
                        // Estimate tokens for CLI (rough approximation)
                        input_tokens: (prompt.len() / 4) as u32,
                        output_tokens: 100, // Default estimate
                    },
                })
            }
        }
    }
}
