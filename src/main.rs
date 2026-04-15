#![allow(dead_code)]

mod auth;
mod cli;
mod config;
mod context;
mod git;
mod models;
mod output;
mod provider;
mod review;

use clap::Parser;

use cli::{Cli, Command, OutputFormat};
use models::{Result, Severity};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    match &cli.command {
        Some(Command::Init) => {
            let path = std::path::Path::new(".revue.toml");
            if path.exists() {
                eprintln!(".revue.toml already exists");
                return Ok(());
            }
            std::fs::write(path, config::default_config_toml())?;
            eprintln!("Created .revue.toml");
            return Ok(());
        }
        Some(Command::Login { provider }) => {
            auth::login(provider)?;
            return Ok(());
        }
        Some(Command::Logout { provider }) => {
            auth::logout(provider)?;
            return Ok(());
        }
        None => {}
    }

    // Resolve provider
    let provider_name = cli
        .provider
        .unwrap_or_else(|| auth::get_default_provider());

    // Resolve API key
    let api_key = auth::get_api_key(&provider_name)
        .or_else(|| {
            // Fallback: try legacy config for Claude
            if provider_name == "claude" {
                let repo = git::open_repo().ok()?;
                let root = git::repo_root(&repo).ok()?;
                let cfg = config::load_config(&root);
                config::resolve_api_key(&cfg).ok()
            } else {
                None
            }
        })
        .ok_or_else(|| {
            models::RevueError::MissingApiKey
        })?;

    // Create provider
    let prov = provider::create_provider(&provider_name, &api_key)?;

    // Resolve model
    let repo = git::open_repo()?;
    let root = git::repo_root(&repo)?;
    let cfg = config::load_config(&root);
    let model = cli
        .model
        .or(cfg.model.clone())
        .unwrap_or_else(|| prov.default_model().to_string());

    // Get diffs
    let mut diffs = if let Some(ref range) = cli.commit {
        git::commit_range_diff(&repo, range)?
    } else {
        git::staged_diff(&repo)?
    };

    // Filter by --file if specified
    if let Some(ref file_filter) = cli.file {
        diffs.retain(|d| d.path.contains(file_filter));
    }

    // Filter by ignore patterns
    if !cfg.ignore_patterns.is_empty() {
        diffs.retain(|d| {
            !cfg.ignore_patterns
                .iter()
                .any(|pattern| matches_glob(pattern, &d.path))
        });
    }

    if diffs.is_empty() {
        output::render_header();
        if cli.commit.is_some() {
            eprintln!("No changes found in the specified commit range.");
        } else {
            eprintln!("No staged changes found. Stage files with `git add` first, or use --commit to review a commit range.");
        }
        return Ok(());
    }

    let context = context::build_context(&repo, &diffs, &cfg)?;

    output::render_header();
    let mut spinner =
        output::SpinnerHandle::start(&format!("Reviewing {} file(s)...", diffs.len()));

    let result = prov.review(&diffs, &context, &model).await?;

    spinner.stop();

    match cli.format {
        OutputFormat::Pretty => output::render_review(&result, cli.severity),
        OutputFormat::Json => output::render_json(&result),
    }

    if result.issues.iter().any(|i| i.severity == Severity::Critical) {
        std::process::exit(1);
    }

    Ok(())
}

fn matches_glob(pattern: &str, path: &str) -> bool {
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0].trim_end_matches('/');
            let suffix = parts[1].trim_start_matches('/');
            let prefix_match = prefix.is_empty() || path.starts_with(prefix);
            let suffix_match = suffix.is_empty() || path.ends_with(suffix);
            return prefix_match && suffix_match;
        }
    }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        return path.ends_with(ext);
    }
    path == pattern
}
