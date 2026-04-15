use git2::Repository;

use crate::config::RevueConfig;
use crate::models::{FileContext, FileDiff, RepoContext, Result};

const DEFAULT_MAX_TOKENS: usize = 80_000;
const RESERVED_TOKENS: usize = 28_000;

/// Estimate token count from text (roughly 1 token per 4 chars)
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Build the full context for the review
pub fn build_context(
    repo: &Repository,
    diffs: &[FileDiff],
    config: &RevueConfig,
) -> Result<RepoContext> {
    let max_tokens = config.max_context_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let budget = max_tokens.saturating_sub(RESERVED_TOKENS);

    // Gather changed file contents
    let mut changed_files = Vec::new();
    let mut tokens_used: usize = 0;

    for diff in diffs {
        // Try to read the full file content from HEAD (for context)
        let content = crate::git::read_file_content(repo, &diff.path)
            .unwrap_or_else(|_| diff.raw_patch.clone());

        let token_est = estimate_tokens(&content);
        if tokens_used + token_est > budget {
            // If even the patch alone fits, include just the patch
            let patch_tokens = estimate_tokens(&diff.raw_patch);
            if tokens_used + patch_tokens <= budget {
                tokens_used += patch_tokens;
                changed_files.push(FileContext {
                    path: diff.path.clone(),
                    content: diff.raw_patch.clone(),
                    token_estimate: patch_tokens,
                });
            }
            continue;
        }

        tokens_used += token_est;
        changed_files.push(FileContext {
            path: diff.path.clone(),
            content,
            token_estimate: token_est,
        });
    }

    // Find related files
    let mut related_files = Vec::new();
    let remaining_budget = budget.saturating_sub(tokens_used);

    if remaining_budget > 1000 {
        let mut related_paths = Vec::new();
        for diff in diffs {
            let mut found = find_related_files(repo, &diff.path, &diff.raw_patch);
            related_paths.append(&mut found);
        }
        related_paths.sort();
        related_paths.dedup();

        // Exclude already-included files
        let changed_paths: Vec<&str> = changed_files.iter().map(|f| f.path.as_str()).collect();
        related_paths.retain(|p| !changed_paths.contains(&p.as_str()));

        let mut related_tokens = 0usize;
        for path in related_paths {
            if related_tokens >= remaining_budget {
                break;
            }
            if let Ok(content) = crate::git::read_file_content(repo, &path) {
                let token_est = estimate_tokens(&content);
                if related_tokens + token_est <= remaining_budget {
                    related_tokens += token_est;
                    related_files.push(FileContext {
                        path,
                        content,
                        token_estimate: token_est,
                    });
                }
            }
        }
        tokens_used += related_tokens;
    }

    // Build repo tree
    let tracked = crate::git::list_tracked_files(repo).unwrap_or_default();
    let repo_tree = build_repo_tree(&tracked);

    Ok(RepoContext {
        repo_tree,
        changed_files,
        related_files,
        total_tokens: tokens_used,
    })
}

/// Find files that may be related to the given file based on imports and directory
fn find_related_files(repo: &Repository, file_path: &str, content: &str) -> Vec<String> {
    let mut related = Vec::new();

    // Find files in same directory
    let dir = std::path::Path::new(file_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    if let Ok(tracked) = crate::git::list_tracked_files(repo) {
        for tracked_file in &tracked {
            // Same directory (but not the file itself)
            if tracked_file != file_path {
                if let Some(tracked_dir) = std::path::Path::new(tracked_file).parent() {
                    if tracked_dir.to_string_lossy() == dir {
                        related.push(tracked_file.clone());
                    }
                }
            }
        }
    }

    // Scan for import/use/require statements
    for line in content.lines() {
        let line = line.trim();
        // Rust: use crate::module, mod module
        if line.starts_with("use ") || line.starts_with("mod ") {
            if let Some(module) = extract_module_path(line) {
                related.push(module);
            }
        }
        // JS/TS: import ... from '...' or require('...')
        if line.contains("import ") || line.contains("require(") {
            if let Some(import_path) = extract_import_path(line) {
                related.push(import_path);
            }
        }
    }

    related
}

fn extract_module_path(line: &str) -> Option<String> {
    // Simple heuristic: "use crate::foo::bar" -> "src/foo/bar.rs" or "src/foo.rs"
    let line = line.trim_end_matches(';').trim();
    if let Some(rest) = line.strip_prefix("use crate::") {
        let parts: Vec<&str> = rest.split("::").collect();
        if !parts.is_empty() {
            let path = format!("src/{}.rs", parts[0]);
            return Some(path);
        }
    }
    if let Some(rest) = line.strip_prefix("mod ") {
        let module = rest.trim().trim_end_matches(';');
        return Some(format!("src/{}.rs", module));
    }
    None
}

fn extract_import_path(line: &str) -> Option<String> {
    // Extract path from: import ... from './foo' or require('./foo')
    let line = line.trim();
    for quote in &['\'', '"'] {
        if let Some(start) = line.rfind(*quote) {
            let before = &line[..start];
            if let Some(begin) = before.rfind(*quote) {
                let path = &line[begin + 1..start];
                if path.starts_with('.') {
                    return Some(path.to_string());
                }
            }
        }
    }
    None
}

/// Build a tree-style listing of files
fn build_repo_tree(files: &[String]) -> String {
    if files.is_empty() {
        return String::from("(empty repository)");
    }

    let mut tree = String::new();
    tree.push_str("Repository structure:\n");

    // Group by top-level directory
    let mut dirs: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for file in files {
        let parts: Vec<&str> = file.splitn(2, '/').collect();
        if parts.len() == 2 {
            dirs.entry(parts[0].to_string())
                .or_default()
                .push(parts[1].to_string());
        } else {
            dirs.entry(".".to_string())
                .or_default()
                .push(file.clone());
        }
    }

    for (dir, entries) in &dirs {
        if dir == "." {
            for entry in entries {
                tree.push_str(&format!("  {}\n", entry));
            }
        } else {
            tree.push_str(&format!("  {}/\n", dir));
            // Show up to 20 entries per dir to keep it manageable
            for (i, entry) in entries.iter().enumerate() {
                if i >= 20 {
                    tree.push_str(&format!("    ... and {} more\n", entries.len() - 20));
                    break;
                }
                tree.push_str(&format!("    {}\n", entry));
            }
        }
    }

    tree
}
