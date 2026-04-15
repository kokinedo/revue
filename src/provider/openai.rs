use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::Provider;
use crate::models::{FileDiff, RepoContext, Result, ReviewResult, RevueError};
use crate::review::{build_system_prompt, build_user_message, parse_review_response};

pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
}

impl OpenAIProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    fmt_type: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str { "openai" }
    fn default_model(&self) -> &str { "gpt-4o" }

    async fn review(&self, diffs: &[FileDiff], context: &RepoContext, model: &str) -> Result<ReviewResult> {
        let system = build_system_prompt();
        let user_msg = build_user_message(diffs, context);

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage { role: "system".to_string(), content: system },
                ChatMessage { role: "user".to_string(), content: user_msg },
            ],
            response_format: ResponseFormat { fmt_type: "json_object".to_string() },
        };

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(RevueError::Api(format!("OpenAI API error {}: {}", status, body)));
        }

        let resp: ChatResponse = serde_json::from_str(&body)
            .map_err(|e| RevueError::Api(format!("Failed to parse response: {}", e)))?;

        let raw = resp.choices.into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| RevueError::Api("No content in response".into()))?;

        parse_review_response(&raw, diffs.len(), model)
    }
}
