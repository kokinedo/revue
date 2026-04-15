use std::path::PathBuf;
use std::collections::HashMap;

use git2::{DiffFormat, DiffOptions, Repository};

use crate::models::{DiffHunk, DiffLine, FileDiff, FileStatus, Result, RevueError};

/// Open the repository by discovering from the current working directory
pub fn open_repo() -> Result<Repository> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    Ok(repo)
}

/// Get the repository root path
pub fn repo_root(repo: &Repository) -> Result<PathBuf> {
    repo.workdir()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| RevueError::Git(git2::Error::from_str("bare repository")))
}

/// Get diff for staged changes (index vs HEAD)
pub fn staged_diff(repo: &Repository) -> Result<Vec<FileDiff>> {
    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(_) => None, // Empty repo, no HEAD yet
    };

    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
    build_file_diffs(&diff)
}

/// Get diff for a commit range like "A..B"
pub fn commit_range_diff(repo: &Repository, range: &str) -> Result<Vec<FileDiff>> {
    let parts: Vec<&str> = range.split("..").collect();
    if parts.len() != 2 {
        return Err(RevueError::Other(format!(
            "Invalid commit range '{}'. Expected format: A..B",
            range
        )));
    }

    let from_obj = repo.revparse_single(parts[0])?;
    let to_obj = repo.revparse_single(parts[1])?;

    let from_tree = from_obj.peel_to_tree()?;
    let to_tree = to_obj.peel_to_tree()?;

    let mut opts = DiffOptions::new();
    let diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut opts))?;
    build_file_diffs(&diff)
}

/// Read file content from HEAD tree
pub fn read_file_content(repo: &Repository, path: &str) -> Result<String> {
    let head = repo.head()?;
    let tree = head.peel_to_tree()?;
    let entry = tree.get_path(std::path::Path::new(path))?;
    let blob = repo.find_blob(entry.id())?;
    let content = std::str::from_utf8(blob.content())
        .map_err(|e| RevueError::Other(format!("UTF-8 error reading {}: {}", path, e)))?;
    Ok(content.to_string())
}

/// List all tracked files in the index
pub fn list_tracked_files(repo: &Repository) -> Result<Vec<String>> {
    let index = repo.index()?;
    let files: Vec<String> = index
        .iter()
        .map(|entry| {
            String::from_utf8_lossy(&entry.path).to_string()
        })
        .collect();
    Ok(files)
}

/// Build FileDiff structs from a git2 Diff
fn build_file_diffs(diff: &git2::Diff) -> Result<Vec<FileDiff>> {
    // First pass: collect file metadata
    let mut files: Vec<FileDiff> = Vec::new();
    let num_deltas = diff.deltas().len();

    for i in 0..num_deltas {
        let delta = diff.get_delta(i).unwrap();
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let status = match delta.status() {
            git2::Delta::Added => FileStatus::Added,
            git2::Delta::Deleted => FileStatus::Deleted,
            git2::Delta::Renamed => FileStatus::Renamed,
            _ => FileStatus::Modified,
        };

        files.push(FileDiff {
            path,
            status,
            hunks: Vec::new(),
            raw_patch: String::new(),
        });
    }

    // Second pass: collect patch text using print callback
    let mut current_file_idx: Option<usize> = None;
    let mut file_patches: HashMap<usize, String> = HashMap::new();
    let mut file_hunks: HashMap<usize, Vec<DiffHunk>> = HashMap::new();

    diff.print(DiffFormat::Patch, |delta, hunk, line| {
        // Determine which file this belongs to
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let file_idx = files.iter().position(|f| f.path == path).unwrap_or(0);

        // Track file transitions for hunk grouping
        let line_origin = line.origin();
        let content = std::str::from_utf8(line.content()).unwrap_or("").to_string();

        // Append to raw patch
        let patch = file_patches.entry(file_idx).or_default();
        match line_origin {
            '+' | '-' | ' ' => {
                patch.push(line_origin);
                patch.push_str(&content);
            }
            'H' => {
                patch.push_str(&content);
            }
            _ => {}
        }

        // Build hunks
        if line_origin == 'H' {
            // New hunk header
            let hunks = file_hunks.entry(file_idx).or_default();
            let header = hunk
                .map(|h| {
                    format!(
                        "@@ -{},{} +{},{} @@",
                        h.old_start(),
                        h.old_lines(),
                        h.new_start(),
                        h.new_lines()
                    )
                })
                .unwrap_or_default();
            hunks.push(DiffHunk {
                header,
                lines: Vec::new(),
            });
            current_file_idx = Some(file_idx);
        } else if matches!(line_origin, '+' | '-' | ' ') {
            if current_file_idx != Some(file_idx) {
                // Create a default hunk if we haven't seen a header
                let hunks = file_hunks.entry(file_idx).or_default();
                if hunks.is_empty() {
                    hunks.push(DiffHunk {
                        header: String::new(),
                        lines: Vec::new(),
                    });
                }
                current_file_idx = Some(file_idx);
            }
            let hunks = file_hunks.entry(file_idx).or_default();
            if let Some(last_hunk) = hunks.last_mut() {
                last_hunk.lines.push(DiffLine {
                    origin: line_origin,
                    content,
                });
            }
        }

        true
    })?;

    // Merge collected data back into files
    for (idx, file) in files.iter_mut().enumerate() {
        if let Some(patch) = file_patches.remove(&idx) {
            file.raw_patch = patch;
        }
        if let Some(hunks) = file_hunks.remove(&idx) {
            file.hunks = hunks;
        }
    }

    Ok(files)
}
