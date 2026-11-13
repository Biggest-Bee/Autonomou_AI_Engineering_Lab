// Security & Configuration Module
// Rate limiting plus app configuration loaded from environment variables

use serde::{Deserialize, Serialize};
use anyhow::Result;

pub mod rate_limiter;

pub use rate_limiter::RateLimiter;
pub use rate_limiter::RequestType;

/// Application configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub system_prompt: String,
    pub production_mode: bool,
    pub default_terminal: String,
}

impl SecurityConfig {
    pub fn from_env() -> Result<Self> {
        let system_prompt = std::env::var("SYSTEM_PROMPT")
            .unwrap_or_else(|_| "***REMOVED***".to_string());

        let production_mode = std::env::var("PRODUCTION_MODE")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let default_terminal = std::env::var("DEFAULT_TERMINAL")
            .unwrap_or_else(|_| "powershell".to_string());

        Ok(Self {
            system_prompt,
            production_mode,
            default_terminal,
        })
    }
}

/// Structured message format for AI interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredMessage {
    pub role: String,
    pub content: String,
}

impl StructuredMessage {
    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
        }
    }

    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Message container for structured AI communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContainer {
    pub messages: Vec<StructuredMessage>,
}

impl MessageContainer {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add_system(&mut self, content: String) {
        self.messages.push(StructuredMessage::system(content));
    }

    pub fn add_user(&mut self, content: String) {
        self.messages.push(StructuredMessage::user(content));
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

impl Default for MessageContainer {
    fn default() -> Self {
        Self::new()
    }
}
