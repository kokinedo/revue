#![allow(dead_code)]

mod cli;
mod config;
mod context;
mod git;
mod models;
mod output;
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

    // Handle init subcommand
    if let Some(Command::Init) = &cli.command {
        let path = std::path::Path::new(".revue.toml");
        if path.exists() {
            eprintln!(".revue.toml already exists");
            return Ok(());
        }
        std::fs::write(path, config::default_config_toml())?;
        eprintln!("Created .revue.toml — edit it to add your API key or set ANTHROPIC_API_KEY");
        return Ok(());
    }

    let repo = git::open_repo()?;
    let root = git::repo_root(&repo)?;
    let config = config::load_config(&root);
    let api_key = config::resolve_api_key(&config)?;
    let model = cli
        .model
        .or(config.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".into());

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
    if !config.ignore_patterns.is_empty() {
        diffs.retain(|d| {
            !config
                .ignore_patterns
                .iter()
                .any(|pattern| matches_glob(pattern, &d.path))
        });
    }

    // Check if there are any diffs to review
    if diffs.is_empty() {
        output::render_header();
        if cli.commit.is_some() {
            eprintln!("No changes found in the specified commit range.");
        } else {
            eprintln!("No staged changes found. Stage files with `git add` first, or use --commit to review a commit range.");
        }
        return Ok(());
    }

    // Build context
    let context = context::build_context(&repo, &diffs, &config)?;

    // Run review
    output::render_header();
    let mut spinner =
        output::SpinnerHandle::start(&format!("Reviewing {} file(s)...", diffs.len()));

    let result = review::run_review(&diffs, &context, &api_key, &model).await?;

    spinner.stop();

    // Render output
    match cli.format {
        OutputFormat::Pretty => output::render_review(&result, cli.severity),
        OutputFormat::Json => output::render_json(&result),
    }

    // Exit with code 1 if any critical issues found (CI-friendly)
    if result
        .issues
        .iter()
        .any(|i| i.severity == Severity::Critical)
    {
        std::process::exit(1);
    }

    Ok(())
}

/// Simple glob matching (supports * and **)
fn matches_glob(pattern: &str, path: &str) -> bool {
    if pattern.contains("**") {
        // ** matches any number of path segments
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
        // *.ext matches any file with that extension
        let ext = &pattern[1..];
        return path.ends_with(ext);
    }
    // Exact match
    path == pattern
}
