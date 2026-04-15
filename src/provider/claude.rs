use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::Provider;
use crate::models::{FileDiff, RepoContext, Result, ReviewResult, RevueError};
use crate::review::{build_system_prompt, build_user_message, parse_review_response};

pub struct ClaudeProvider {
    client: reqwest::Client,
    api_key: String,
}

impl ClaudeProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn name(&self) -> &str { "claude" }
    fn default_model(&self) -> &str { "claude-sonnet-4-20250514" }

    async fn review(&self, diffs: &[FileDiff], context: &RepoContext, model: &str) -> Result<ReviewResult> {
        let system = build_system_prompt();
        let user_msg = build_user_message(diffs, context);

        let request = ApiRequest {
            model: model.to_string(),
            max_tokens: 8192,
            system,
            messages: vec![ApiMessage {
                role: "user".to_string(),
                content: user_msg,
            }],
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(RevueError::Api(format!("Claude API error {}: {}", status, body)));
        }

        let api_resp: ApiResponse = serde_json::from_str(&body)
            .map_err(|e| RevueError::Api(format!("Failed to parse response: {}", e)))?;

        let raw = api_resp.content.into_iter()
            .find_map(|b| b.text)
            .ok_or_else(|| RevueError::Api("No text content in response".into()))?;

        parse_review_response(&raw, diffs.len(), model)
    }
}
