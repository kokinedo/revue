use serde::{Deserialize, Serialize};

use crate::models::{FileDiff, RepoContext, Result, ReviewResult, RevueError};

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// Run a full code review
pub async fn run_review(
    diffs: &[FileDiff],
    context: &RepoContext,
    api_key: &str,
    model: &str,
) -> Result<ReviewResult> {
    let system = build_system_prompt();
    let user_msg = build_user_message(diffs, context);

    let raw = call_claude(&system, &user_msg, api_key, model).await?;
    let files_reviewed = diffs.len();
    parse_review_response(&raw, files_reviewed, model)
}

fn build_system_prompt() -> String {
    r#"You are a senior software engineer conducting a thorough code review. Analyze the provided code changes and identify issues across these categories: security, performance, bug, style, maintainability.

For each issue, assess severity:
- critical: Security vulnerabilities, data loss risks, crashes
- warning: Bugs, performance issues, logic errors
- suggestion: Code improvements, better patterns
- info: Style notes, minor observations

Respond ONLY with valid JSON matching this exact schema:
{
  "summary": "Brief overall assessment of the changes",
  "issues": [
    {
      "severity": "critical|warning|suggestion|info",
      "category": "security|performance|bug|style|maintainability",
      "file": "path/to/file.rs",
      "line_start": 42,
      "line_end": 45,
      "title": "Short issue title",
      "description": "Detailed explanation of the issue",
      "suggestion": "Optional suggested fix or improvement"
    }
  ]
}

Guidelines:
- Be thorough but avoid false positives
- Provide actionable, specific feedback
- Include line numbers when possible
- Focus on substantive issues over nitpicks
- If the code looks good, say so with an empty issues array
- line_start and line_end can be null if not applicable
- suggestion can be null if not applicable"#
        .to_string()
}

fn build_user_message(diffs: &[FileDiff], context: &RepoContext) -> String {
    let mut msg = String::new();

    msg.push_str("# Code Review Request\n\n");

    // Repository structure
    msg.push_str("## Repository Structure\n```\n");
    msg.push_str(&context.repo_tree);
    msg.push_str("```\n\n");

    // Changed files (diffs)
    msg.push_str("## Changes to Review\n\n");
    for diff in diffs {
        msg.push_str(&format!(
            "### {} ({})\n```diff\n{}\n```\n\n",
            diff.path, diff.status, diff.raw_patch
        ));
    }

    // Full file context
    if !context.changed_files.is_empty() {
        msg.push_str("## Full File Context\n\n");
        for file in &context.changed_files {
            msg.push_str(&format!(
                "### {}\n```\n{}\n```\n\n",
                file.path, file.content
            ));
        }
    }

    // Related files
    if !context.related_files.is_empty() {
        msg.push_str("## Related Files (for context)\n\n");
        for file in &context.related_files {
            msg.push_str(&format!(
                "### {}\n```\n{}\n```\n\n",
                file.path, file.content
            ));
        }
    }

    msg
}

async fn call_claude(
    system: &str,
    user_msg: &str,
    api_key: &str,
    model: &str,
) -> Result<String> {
    let client = reqwest::Client::new();

    let request = ApiRequest {
        model: model.to_string(),
        max_tokens: 8192,
        system: system.to_string(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: user_msg.to_string(),
        }],
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        // Try to parse error message
        if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
            if let Some(detail) = err.error {
                return Err(RevueError::Api(
                    detail.message.unwrap_or_else(|| format!("HTTP {}", status)),
                ));
            }
        }
        return Err(RevueError::Api(format!("HTTP {} — {}", status, body)));
    }

    let api_resp: ApiResponse = serde_json::from_str(&body)
        .map_err(|e| RevueError::Api(format!("Failed to parse API response: {}", e)))?;

    api_resp
        .content
        .into_iter()
        .find_map(|block| block.text)
        .ok_or_else(|| RevueError::Api("No text content in API response".into()))
}

fn parse_review_response(
    raw: &str,
    files_reviewed: usize,
    model: &str,
) -> Result<ReviewResult> {
    // Strip markdown code fences if present
    let text = raw.trim();
    let text = if text.starts_with("```") {
        // Remove opening fence (possibly ```json)
        let after_first = text.find('\n').map(|i| &text[i + 1..]).unwrap_or(text);
        // Remove closing fence
        if let Some(end) = after_first.rfind("```") {
            &after_first[..end]
        } else {
            after_first
        }
    } else {
        text
    };

    // Find the outermost JSON object
    let text = text.trim();
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    let mut result: ReviewResult = serde_json::from_str(json_str)?;
    result.files_reviewed = files_reviewed;
    result.model = model.to_string();

    Ok(result)
}
