use serde::{Deserialize, Serialize};

/// Request to Claude Messages API
#[derive(Debug, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

impl MessagesRequest {
    pub fn new(model: String, messages: Vec<Message>) -> Self {
        MessagesRequest {
            model,
            max_tokens: 4096,
            messages,
            system: None,
            temperature: None,
        }
    }

    pub fn with_system(mut self, system: String) -> Self {
        self.system = Some(system);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// Message in conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn user(content: String) -> Self {
        Message {
            role: MessageRole::User,
            content,
        }
    }

    pub fn assistant(content: String) -> Self {
        Message {
            role: MessageRole::Assistant,
            content,
        }
    }
}

/// Message role
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

/// Response from Claude Messages API
#[derive(Debug, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    pub content: Vec<Content>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

impl MessagesResponse {
    /// Get the text content from the response
    pub fn get_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text.clone()),
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Content block in response
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text { text: String },
}

/// Token usage information
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Error response from API
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

/// API error details
#[derive(Debug, Deserialize)]
pub struct ApiError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Summary request for processing GitHub activity
#[derive(Debug)]
pub struct SummaryRequest {
    pub activities: String,
    pub context: Option<String>,
    pub importance_level: ImportanceLevel,
}

/// Importance level for summarization
#[derive(Debug, Clone, Copy)]
pub enum ImportanceLevel {
    High,
    Medium,
    Low,
}

impl ImportanceLevel {
    pub fn model(&self, config: &crate::config::ClaudeConfig) -> String {
        match self {
            ImportanceLevel::High | ImportanceLevel::Medium => config.primary_model.clone(),
            ImportanceLevel::Low => config.secondary_model.clone(),
        }
    }

    pub fn max_tokens(&self) -> u32 {
        match self {
            ImportanceLevel::High => 8000,
            ImportanceLevel::Medium => 4000,
            ImportanceLevel::Low => 2000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_request_builder() {
        let request = MessagesRequest::new(
            "claude-3-5-sonnet-20241022".to_string(),
            vec![Message::user("Hello".to_string())],
        )
        .with_system("You are a helpful assistant".to_string())
        .with_max_tokens(1000)
        .with_temperature(0.7);

        assert_eq!(request.model, "claude-3-5-sonnet-20241022");
        assert_eq!(request.max_tokens, 1000);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(
            request.system,
            Some("You are a helpful assistant".to_string())
        );
    }

    #[test]
    fn test_message_constructors() {
        let user_msg = Message::user("User message".to_string());
        assert!(matches!(user_msg.role, MessageRole::User));
        assert_eq!(user_msg.content, "User message");

        let assistant_msg = Message::assistant("Assistant message".to_string());
        assert!(matches!(assistant_msg.role, MessageRole::Assistant));
        assert_eq!(assistant_msg.content, "Assistant message");
    }
}
