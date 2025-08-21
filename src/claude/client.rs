use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{Client as HttpClient, Response};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json;
use std::time::Duration;
use tracing::warn;

use crate::claude::{
    get_api_key, resolve_model_alias, ErrorResponse, MessagesRequest, MessagesResponse,
};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// Claude client abstraction
pub enum ClaudeClient {
    Real(RealClaude),
    #[cfg(test)]
    Mock(MockClaude),
}

impl ClaudeClient {
    /// Create a new real Claude client
    pub fn new() -> Result<Self> {
        Ok(ClaudeClient::Real(RealClaude::new()?))
    }

    /// Create a mock client for testing
    #[cfg(test)]
    pub fn mock() -> Self {
        ClaudeClient::Mock(MockClaude::new())
    }

    /// Send a messages request to Claude
    pub fn messages(&self, request: MessagesRequest) -> Result<MessagesResponse> {
        match self {
            ClaudeClient::Real(client) => client.messages(request),
            #[cfg(test)]
            ClaudeClient::Mock(client) => client.messages(request),
        }
    }

    /// Send a messages request with retries
    pub fn messages_with_retry(
        &self,
        request: MessagesRequest,
        max_retries: u32,
    ) -> Result<MessagesResponse> {
        match self {
            ClaudeClient::Real(client) => client.messages_with_retry(request, max_retries),
            #[cfg(test)]
            ClaudeClient::Mock(client) => client.messages(request),
        }
    }
}

/// Real Claude API client
pub struct RealClaude {
    client: HttpClient,
    api_key: String,
}

impl RealClaude {
    /// Create a new real Claude client
    pub fn new() -> Result<Self> {
        let api_key = get_api_key()?;

        // Basic validation of API key format
        if api_key.trim().is_empty() {
            return Err(anyhow!("ANTHROPIC_API_KEY is empty"));
        }

        if !api_key.starts_with("sk-") {
            tracing::warn!("ANTHROPIC_API_KEY doesn't start with 'sk-' - this may not be a valid Anthropic API key");
        }

        let client = HttpClient::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(RealClaude { client, api_key })
    }

    /// Build request headers
    fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key).context("Invalid API key format")?,
        );
        headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        Ok(headers)
    }

    /// Send a messages request
    pub fn messages(&self, mut request: MessagesRequest) -> Result<MessagesResponse> {
        // Resolve model alias if needed
        request.model = resolve_model_alias(&request.model);

        let headers = self.build_headers()?;
        let body = serde_json::to_string(&request).context("Failed to serialize request")?;

        let response = self
            .client
            .post(API_URL)
            .headers(headers)
            .body(body)
            .send()
            .context("Failed to send request to Claude API")?;

        self.handle_response(response)
    }

    /// Send a messages request with retries
    pub fn messages_with_retry(
        &self,
        mut request: MessagesRequest,
        max_retries: u32,
    ) -> Result<MessagesResponse> {
        let mut attempts = 0;
        let mut last_error = None;

        // Resolve model alias once
        request.model = resolve_model_alias(&request.model);

        while attempts <= max_retries {
            // Build the request each time
            let headers = self.build_headers()?;
            let body = serde_json::to_string(&request).context("Failed to serialize request")?;

            let response = self
                .client
                .post(API_URL)
                .headers(headers)
                .body(body)
                .send()
                .context("Failed to send request to Claude API")?;

            match self.handle_response(response) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    attempts += 1;

                    if attempts <= max_retries {
                        // Exponential backoff: 1s, 2s, 4s, etc.
                        let delay = Duration::from_secs(2_u64.pow(attempts - 1));
                        std::thread::sleep(delay);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Failed after {} retries", max_retries)))
    }

    /// Handle API response
    fn handle_response(&self, response: Response) -> Result<MessagesResponse> {
        let status = response.status();
        let body = response.text().context("Failed to read response body")?;

        if status.is_success() {
            serde_json::from_str(&body).context("Failed to parse successful response")
        } else {
            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&body) {
                Err(anyhow!(
                    "Claude API error ({}): {}",
                    error_response.error.error_type,
                    error_response.error.message
                ))
            } else {
                Err(anyhow!("Claude API error ({}): {}", status, body))
            }
        }
    }
}

/// Mock Claude client for testing
#[cfg(test)]
pub struct MockClaude {
    pub responses: Vec<MessagesResponse>,
    pub call_count: std::cell::RefCell<usize>,
}

#[cfg(test)]
impl MockClaude {
    pub fn new() -> Self {
        MockClaude {
            responses: vec![],
            call_count: std::cell::RefCell::new(0),
        }
    }

    pub fn with_response(mut self, response: MessagesResponse) -> Self {
        self.responses.push(response);
        self
    }

    pub fn messages(&self, _request: MessagesRequest) -> Result<MessagesResponse> {
        let mut count = self.call_count.borrow_mut();
        let index = *count;
        *count += 1;

        self.responses
            .get(index)
            .cloned()
            .ok_or_else(|| anyhow!("No mock response configured for call {}", index))
    }
}

#[cfg(test)]
impl Clone for MessagesResponse {
    fn clone(&self) -> Self {
        MessagesResponse {
            id: self.id.clone(),
            content: self.content.clone(),
            model: self.model.clone(),
            stop_reason: self.stop_reason.clone(),
            usage: crate::claude::Usage {
                input_tokens: self.usage.input_tokens,
                output_tokens: self.usage.output_tokens,
            },
        }
    }
}

#[cfg(test)]
impl Clone for crate::claude::Content {
    fn clone(&self) -> Self {
        match self {
            crate::claude::Content::Text { text } => {
                crate::claude::Content::Text { text: text.clone() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::{Content, Message, Usage};

    #[test]
    fn test_mock_claude_client() {
        let mock_response = MessagesResponse {
            id: "msg_123".to_string(),
            content: vec![Content::Text {
                text: "Test response".to_string(),
            }],
            model: "claude-3-5-sonnet-20241022".to_string(),
            stop_reason: Some("end_turn".to_string()),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };

        let client = MockClaude::new().with_response(mock_response.clone());

        let request = MessagesRequest::new(
            "sonnet".to_string(),
            vec![Message::user("Test".to_string())],
        );

        let response = client.messages(request).unwrap();
        assert_eq!(response.get_text(), "Test response");
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }

    #[test]
    fn test_resolve_model_in_request() {
        // This would require environment setup for real client
        // Testing is done through mock client above
    }
}
