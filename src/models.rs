use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Suggestion,
    Warning,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Suggestion => write!(f, "suggestion"),
            Severity::Warning => write!(f, "warning"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Security,
    Performance,
    Bug,
    Style,
    Maintainability,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Security => write!(f, "security"),
            Category::Performance => write!(f, "performance"),
            Category::Bug => write!(f, "bug"),
            Category::Style => write!(f, "style"),
            Category::Maintainability => write!(f, "maintainability"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub severity: Severity,
    pub category: Category,
    pub file: String,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub title: String,
    pub description: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
    #[serde(skip)]
    pub files_reviewed: usize,
    #[serde(skip)]
    pub model: String,
}

// Git-related types (not serialized to API)
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub hunks: Vec<DiffHunk>,
    pub raw_patch: String,
}

#[derive(Debug, Clone)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::Added => write!(f, "added"),
            FileStatus::Modified => write!(f, "modified"),
            FileStatus::Deleted => write!(f, "deleted"),
            FileStatus::Renamed => write!(f, "renamed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub origin: char,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct FileContext {
    pub path: String,
    pub content: String,
    pub token_estimate: usize,
}

pub struct RepoContext {
    pub repo_tree: String,
    pub changed_files: Vec<FileContext>,
    pub related_files: Vec<FileContext>,
    pub total_tokens: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum RevueError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("No API key found. Set ANTHROPIC_API_KEY or add api_key to .revue.toml")]
    MissingApiKey,
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config error: {0}")]
    Config(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RevueError>;
