use std::path::Path;

use serde::Deserialize;

use crate::models::{Result, RevueError, Severity};

#[derive(Debug, Deserialize, Default)]
pub struct RevueConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub default_severity: Option<Severity>,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
    pub max_context_tokens: Option<usize>,
    pub custom_instructions: Option<String>,
}

/// Load config from .revue.toml in repo root, merging with ~/.config/revue/config.toml
pub fn load_config(repo_root: &Path) -> RevueConfig {
    // Try repo-level config first
    let repo_config_path = repo_root.join(".revue.toml");
    if let Ok(contents) = std::fs::read_to_string(&repo_config_path) {
        if let Ok(config) = toml::from_str::<RevueConfig>(&contents) {
            return config;
        }
    }

    // Fall back to user-level config
    if let Some(config_dir) = dirs::config_dir() {
        let user_config_path = config_dir.join("revue").join("config.toml");
        if let Ok(contents) = std::fs::read_to_string(&user_config_path) {
            if let Ok(config) = toml::from_str::<RevueConfig>(&contents) {
                return config;
            }
        }
    }

    RevueConfig::default()
}

/// Default config template for `revue init`
pub fn default_config_toml() -> &'static str {
    r#"# revue configuration
# See https://github.com/kokinedo/revue for documentation

# Your Anthropic API key (or set ANTHROPIC_API_KEY env var)
# api_key = "sk-ant-..."

# Model to use for reviews
# model = "claude-sonnet-4-20250514"

# Minimum severity to display: info, suggestion, warning, critical
# default_severity = "info"

# File patterns to ignore (glob syntax)
# ignore_patterns = ["*.lock", "*.min.js", "vendor/**"]

# Max context tokens to send (default: 80000)
# max_context_tokens = 80000

# Custom instructions to include in the review prompt
# custom_instructions = "Focus on security issues. This is a financial application."
"#
}

/// Resolve API key from config or environment variable
pub fn resolve_api_key(config: &RevueConfig) -> Result<String> {
    if let Some(ref key) = config.api_key {
        return Ok(key.clone());
    }

    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        return Ok(key);
    }

    Err(RevueError::MissingApiKey)
}
