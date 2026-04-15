use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::Result;

#[derive(Serialize, Deserialize, Default)]
pub struct Credentials {
    pub default_provider: Option<String>,
    pub providers: HashMap<String, ProviderCreds>,
}

#[derive(Serialize, Deserialize)]
pub struct ProviderCreds {
    pub api_key: String,
}

fn env_var_name(provider: &str) -> &str {
    match provider {
        "claude" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        _ => "",
    }
}

fn console_url(provider: &str) -> &str {
    match provider {
        "claude" => "https://console.anthropic.com/settings/keys",
        "openai" => "https://platform.openai.com/api-keys",
        "gemini" => "https://aistudio.google.com/apikey",
        _ => "",
    }
}

fn provider_display_name(provider: &str) -> &str {
    match provider {
        "claude" => "Anthropic (Claude)",
        "openai" => "OpenAI",
        "gemini" => "Google (Gemini)",
        _ => provider,
    }
}

pub fn credentials_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("revue")
        .join("credentials.json")
}

pub fn load_credentials() -> Credentials {
    let path = credentials_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Credentials::default(),
    }
}

pub fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = credentials_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(creds)
        .map_err(|e| crate::models::RevueError::Other(e.to_string()))?;
    std::fs::write(&path, data)?;
    Ok(())
}

pub fn get_api_key(provider: &str) -> Option<String> {
    let creds = load_credentials();
    if let Some(pc) = creds.providers.get(provider) {
        if !pc.api_key.is_empty() {
            return Some(pc.api_key.clone());
        }
    }
    let env = env_var_name(provider);
    if !env.is_empty() {
        if let Ok(val) = std::env::var(env) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

pub fn get_default_provider() -> String {
    let creds = load_credentials();
    creds.default_provider.unwrap_or_else(|| "claude".to_string())
}

pub fn login(provider: &str) -> Result<()> {
    let name = provider_display_name(provider);
    let url = console_url(provider);

    eprintln!("\n  Logging in to {}...\n", name);

    if !url.is_empty() {
        eprintln!("  Opening {}", url);
        let _ = open::that(url);
    }

    eprint!("\n  Paste your API key: ");
    io::stderr().flush()?;

    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    let key = key.trim().to_string();

    if key.is_empty() {
        eprintln!("  No key provided. Aborting.");
        return Ok(());
    }

    let mut creds = load_credentials();
    creds.providers.insert(provider.to_string(), ProviderCreds { api_key: key });
    creds.default_provider = Some(provider.to_string());
    save_credentials(&creds)?;

    eprintln!("\n  Logged in to {} successfully.", name);
    eprintln!("  Credentials saved to {}\n", credentials_path().display());
    Ok(())
}

pub fn logout(provider: &str) -> Result<()> {
    let mut creds = load_credentials();
    if creds.providers.remove(provider).is_some() {
        save_credentials(&creds)?;
        eprintln!("\n  Logged out of {}.\n", provider_display_name(provider));
    } else {
        eprintln!("\n  No stored credentials for {}.\n", provider);
    }
    Ok(())
}
