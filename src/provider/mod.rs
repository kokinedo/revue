pub mod claude;
pub mod openai;
pub mod gemini;

use async_trait::async_trait;

use crate::models::{FileDiff, RepoContext, Result, ReviewResult, RevueError};

#[async_trait]
pub trait Provider {
    fn name(&self) -> &str;
    fn default_model(&self) -> &str;
    async fn review(&self, diffs: &[FileDiff], context: &RepoContext, model: &str) -> Result<ReviewResult>;
}

pub fn create_provider(name: &str, api_key: &str) -> Result<Box<dyn Provider>> {
    match name {
        "claude" => Ok(Box::new(claude::ClaudeProvider::new(api_key))),
        "openai" => Ok(Box::new(openai::OpenAIProvider::new(api_key))),
        "gemini" => Ok(Box::new(gemini::GeminiProvider::new(api_key))),
        _ => Err(RevueError::Other(format!("Unknown provider: {}. Choose: claude, openai, gemini", name))),
    }
}
