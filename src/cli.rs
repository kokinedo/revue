use clap::{Parser, Subcommand, ValueEnum};

use crate::models::Severity;

#[derive(Parser)]
#[command(name = "revue", about = "AI-powered code review", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Commit range to review (e.g. HEAD~3..HEAD)
    #[arg(long)]
    pub commit: Option<String>,

    /// Review only a specific file
    #[arg(long)]
    pub file: Option<String>,

    /// Minimum severity to display
    #[arg(long, default_value = "info")]
    pub severity: Severity,

    /// Output format
    #[arg(long, default_value = "pretty")]
    pub format: OutputFormat,

    /// Model to use for review
    #[arg(long)]
    pub model: Option<String>,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a .revue.toml config file
    Init,
}

#[derive(ValueEnum, Clone)]
pub enum OutputFormat {
    Pretty,
    Json,
}
