use revue::models::{ReviewResult, ReviewIssue, Severity, Category};
use revue::output;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("full");

    match mode {
        "clean" => clean_review(),
        "compact" => compact_review(),
        _ => full_review(),
    }
}

fn full_review() {
    let result = ReviewResult {
        summary: "Generally clean changes with a critical security issue in the API handler that must be fixed before merge. Two performance improvements recommended.".to_string(),
        issues: vec![
            ReviewIssue {
                severity: Severity::Critical,
                category: Category::Security,
                file: "src/api/handlers.rs".to_string(),
                line_start: Some(42),
                line_end: Some(47),
                title: "SQL injection via unsanitized user input".to_string(),
                description: "User-provided search parameter is interpolated directly into the SQL query string without parameterization. An attacker could inject arbitrary SQL through the search field.".to_string(),
                suggestion: Some("let results = sqlx::query(\"SELECT * FROM users WHERE name = $1\")\n    .bind(&search_param)\n    .fetch_all(&pool)\n    .await?;".to_string()),
            },
            ReviewIssue {
                severity: Severity::Warning,
                category: Category::Performance,
                file: "src/services/processor.rs".to_string(),
                line_start: Some(128),
                line_end: Some(145),
                title: "Unnecessary clone in hot loop".to_string(),
                description: "The data vector is cloned on every iteration of the processing loop. Since the original is not used after this point, consider using std::mem::take or passing ownership directly.".to_string(),
                suggestion: Some("let batch = std::mem::take(&mut pending_data);".to_string()),
            },
            ReviewIssue {
                severity: Severity::Suggestion,
                category: Category::Style,
                file: "src/config.rs".to_string(),
                line_start: Some(15),
                line_end: Some(15),
                title: "Unused import".to_string(),
                description: "The std::collections::BTreeMap import is no longer used after the refactor in this PR.".to_string(),
                suggestion: None,
            },
            ReviewIssue {
                severity: Severity::Info,
                category: Category::Maintainability,
                file: "src/lib.rs".to_string(),
                line_start: Some(89),
                line_end: Some(102),
                title: "Consider extracting validation logic".to_string(),
                description: "The validation block handles three separate concerns (format, range, permissions). Extracting into dedicated functions would improve testability.".to_string(),
                suggestion: None,
            },
        ],
        files_reviewed: 5,
        model: "claude-sonnet-4-20250514".to_string(),
    };

    output::render_header();
    output::render_review(&result, Severity::Info);
}

fn compact_review() {
    let result = ReviewResult {
        summary: "Critical security issue found. One performance improvement recommended.".to_string(),
        issues: vec![
            ReviewIssue {
                severity: Severity::Critical,
                category: Category::Security,
                file: "src/api/handlers.rs".to_string(),
                line_start: Some(42),
                line_end: Some(47),
                title: "SQL injection via unsanitized user input".to_string(),
                description: "User-provided search parameter is interpolated directly into the SQL query without parameterization.".to_string(),
                suggestion: Some("sqlx::query(\"SELECT * FROM users WHERE name = $1\").bind(&param)".to_string()),
            },
            ReviewIssue {
                severity: Severity::Warning,
                category: Category::Performance,
                file: "src/services/processor.rs".to_string(),
                line_start: Some(128),
                line_end: Some(145),
                title: "Unnecessary clone in hot loop".to_string(),
                description: "Data vector cloned on every iteration. Use std::mem::take instead.".to_string(),
                suggestion: None,
            },
        ],
        files_reviewed: 5,
        model: "claude-sonnet-4-20250514".to_string(),
    };

    output::render_header();
    output::render_review(&result, Severity::Info);
}

fn clean_review() {
    let result = ReviewResult {
        summary: "Clean, well-structured changes. Good test coverage and consistent code style throughout. No issues detected.".to_string(),
        issues: vec![],
        files_reviewed: 3,
        model: "claude-sonnet-4-20250514".to_string(),
    };

    output::render_header();
    output::render_review(&result, Severity::Info);
}
