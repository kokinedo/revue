use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::Provider;
use crate::models::{FileDiff, RepoContext, Result, ReviewResult, RevueError};
use crate::review::{build_system_prompt, build_user_message, parse_review_response};

pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
}

impl GeminiProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.to_string(),
        }
    }
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction")]
    system_instruction: GeminiContent,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    text: Option<String>,
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str { "gemini" }
    fn default_model(&self) -> &str { "gemini-2.0-flash" }

    async fn review(&self, diffs: &[FileDiff], context: &RepoContext, model: &str) -> Result<ReviewResult> {
        let system = build_system_prompt();
        let user_msg = build_user_message(diffs, context);

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: user_msg }],
            }],
            system_instruction: GeminiContent {
                parts: vec![GeminiPart { text: system }],
            },
            generation_config: GenerationConfig {
                response_mime_type: "application/json".to_string(),
            },
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1/models/{}:generateContent?key={}",
            model, self.api_key
        );

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(RevueError::Api(format!("Gemini API error {}: {}", status, body)));
        }

        let resp: GeminiResponse = serde_json::from_str(&body)
            .map_err(|e| RevueError::Api(format!("Failed to parse response: {}", e)))?;

        let raw = resp.candidates.into_iter()
            .next()
            .and_then(|c| c.content.parts.into_iter().next())
            .and_then(|p| p.text)
            .ok_or_else(|| RevueError::Api("No content in Gemini response".into()))?;

        parse_review_response(&raw, diffs.len(), model)
    }
}
