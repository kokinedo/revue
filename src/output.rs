use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use comfy_table::{presets::UTF8_FULL, modifiers::UTF8_ROUND_CORNERS, Table, ContentArrangement, Cell, Color as TableColor};
use owo_colors::OwoColorize;
use rattles::prelude::*;

use crate::models::{ReviewIssue, ReviewResult, Severity};

const PANEL_WIDTH: usize = 60;

fn is_tty() -> bool {
    std::io::stdout().is_terminal()
}

fn term_width() -> usize {
    80 // sensible default; could use terminal_size crate for actual width
}

// ── Header Banner ──────────────────────────────────────────

pub fn render_header() {
    if !is_tty() {
        return;
    }

    let width = 43;
    let title = "  revue — AI Code Review";
    let padded = format!("{:<width$}", title, width = width - 2);

    eprintln!();
    eprintln!(
        "{}",
        format!("╭{}╮", "─".repeat(width)).dimmed()
    );
    eprintln!(
        "{}{}{}",
        "│".dimmed(),
        padded.bold().cyan(),
        "│".dimmed()
    );
    eprintln!(
        "{}",
        format!("╰{}╯", "─".repeat(width)).dimmed()
    );
    eprintln!();
}

// ── Spinner ────────────────────────────────────────────────

pub struct SpinnerHandle {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl SpinnerHandle {
    pub fn start(message: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));

        if !is_tty() {
            eprintln!("{}", message);
            return Self {
                stop,
                handle: None,
            };
        }

        let stop_clone = stop.clone();
        let msg = message.to_string();

        let handle = thread::spawn(move || {
            let spinner = dots();
            let mut stderr = std::io::stderr();

            while !stop_clone.load(Ordering::Relaxed) {
                let frame = spinner.current_row();
                let _ = write!(stderr, "\r{}  {} ", frame.cyan().to_string(), msg.dimmed());
                let _ = stderr.flush();
                thread::sleep(std::time::Duration::from_millis(80));
            }

            // Clear the spinner line
            let _ = write!(stderr, "\r{}\r", " ".repeat(msg.len() + 10));
            let _ = stderr.flush();
        });

        Self {
            stop,
            handle: Some(handle),
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

// ── Issue Rendering ────────────────────────────────────────

fn severity_icon(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "CRITICAL",
        Severity::Warning => "WARNING",
        Severity::Suggestion => "SUGGESTION",
        Severity::Info => "INFO",
    }
}

fn severity_emoji(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "\u{1f534}",   // red circle
        Severity::Warning => "\u{1f7e1}",    // yellow circle
        Severity::Suggestion => "\u{1f7e2}", // green circle
        Severity::Info => "\u{1f535}",        // blue circle
    }
}

fn color_text(text: &str, severity: &Severity) -> String {
    if !is_tty() {
        return text.to_string();
    }
    match severity {
        Severity::Critical => text.red().bold().to_string(),
        Severity::Warning => text.yellow().bold().to_string(),
        Severity::Suggestion => text.green().bold().to_string(),
        Severity::Info => text.blue().bold().to_string(),
    }
}

fn color_border(ch: &str, severity: &Severity) -> String {
    if !is_tty() {
        return ch.to_string();
    }
    match severity {
        Severity::Critical => ch.red().to_string(),
        Severity::Warning => ch.yellow().to_string(),
        Severity::Suggestion => ch.green().to_string(),
        Severity::Info => ch.blue().to_string(),
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.len() <= max_width {
            lines.push(line.to_string());
        } else {
            let mut remaining = line;
            while remaining.len() > max_width {
                // Try to break at a word boundary
                let break_at = remaining[..max_width]
                    .rfind(' ')
                    .unwrap_or(max_width);
                lines.push(remaining[..break_at].to_string());
                remaining = remaining[break_at..].trim_start();
            }
            if !remaining.is_empty() {
                lines.push(remaining.to_string());
            }
        }
    }
    lines
}

pub fn render_issue(issue: &ReviewIssue, index: usize, _term_width: usize) {
    let width = PANEL_WIDTH;
    let inner = width - 4; // padding inside box
    let sev = &issue.severity;

    // Top border with severity + category
    let label = format!(
        " {} {} {} {} ",
        severity_emoji(sev),
        severity_icon(sev),
        "──",
        issue.category
    );
    let top_fill = width.saturating_sub(label.len() + 2);
    println!(
        "{}{}{}",
        color_border("╭─", sev),
        color_text(&label, sev),
        color_border(&format!("{}╮", "─".repeat(top_fill)), sev)
    );

    // Empty line
    println!(
        "{}{}{}",
        color_border("│", sev),
        " ".repeat(width),
        color_border("│", sev)
    );

    // Title
    let title_line = format!("  #{} {}", index + 1, issue.title);
    let title_padded = format!("{:<width$}", title_line, width = width);
    println!(
        "{}{}{}",
        color_border("│", sev),
        if is_tty() { title_padded.bold().to_string() } else { title_padded },
        color_border("│", sev)
    );

    // File location
    let loc = if let (Some(start), Some(end)) = (issue.line_start, issue.line_end) {
        if start == end {
            format!("  File: {}:{}", issue.file, start)
        } else {
            format!("  File: {}:{}-{}", issue.file, start, end)
        }
    } else if let Some(start) = issue.line_start {
        format!("  File: {}:{}", issue.file, start)
    } else {
        format!("  File: {}", issue.file)
    };
    let loc_padded = format!("{:<width$}", loc, width = width);
    println!(
        "{}{}{}",
        color_border("│", sev),
        if is_tty() { loc_padded.dimmed().to_string() } else { loc_padded },
        color_border("│", sev)
    );

    // Empty line
    println!(
        "{}{}{}",
        color_border("│", sev),
        " ".repeat(width),
        color_border("│", sev)
    );

    // Description
    let desc_lines = wrap_text(&issue.description, inner);
    for line in &desc_lines {
        let padded = format!("  {:<width$}", line, width = width - 2);
        println!(
            "{}{}{}",
            color_border("│", sev),
            padded,
            color_border("│", sev)
        );
    }

    // Suggestion
    if let Some(ref suggestion) = issue.suggestion {
        println!(
            "{}{}{}",
            color_border("│", sev),
            " ".repeat(width),
            color_border("│", sev)
        );

        let sugg_header = format!("  {}", "Suggestion:");
        let sugg_padded = format!("{:<width$}", sugg_header, width = width);
        println!(
            "{}{}{}",
            color_border("│", sev),
            if is_tty() { sugg_padded.italic().to_string() } else { sugg_padded },
            color_border("│", sev)
        );

        let sugg_lines = wrap_text(suggestion, inner - 4);
        for line in &sugg_lines {
            let content = format!("  {} {}", "┃".dimmed(), line);
            let padded = format!("{:<width$}", content, width = width);
            println!(
                "{}{}{}",
                color_border("│", sev),
                padded,
                color_border("│", sev)
            );
        }
    }

    // Bottom border
    println!(
        "{}",
        color_border(&format!("╰{}╯", "─".repeat(width)), sev)
    );
    println!();
}

// ── Summary Table ──────────────────────────────────────────

pub fn render_summary(result: &ReviewResult) {
    let mut critical = 0;
    let mut warning = 0;
    let mut suggestion = 0;
    let mut info = 0;

    for issue in &result.issues {
        match issue.severity {
            Severity::Critical => critical += 1,
            Severity::Warning => warning += 1,
            Severity::Suggestion => suggestion += 1,
            Severity::Info => info += 1,
        }
    }

    // Summary text
    println!();
    if is_tty() {
        println!("{}", "Summary".bold().underline());
    } else {
        println!("Summary");
    }
    println!();
    println!("  {}", result.summary);
    println!();

    // Stats table
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Severity").fg(TableColor::White),
        Cell::new("Count").fg(TableColor::White),
    ]);

    if critical > 0 {
        table.add_row(vec![
            Cell::new("Critical").fg(TableColor::Red),
            Cell::new(critical).fg(TableColor::Red),
        ]);
    }
    if warning > 0 {
        table.add_row(vec![
            Cell::new("Warning").fg(TableColor::Yellow),
            Cell::new(warning).fg(TableColor::Yellow),
        ]);
    }
    if suggestion > 0 {
        table.add_row(vec![
            Cell::new("Suggestion").fg(TableColor::Green),
            Cell::new(suggestion).fg(TableColor::Green),
        ]);
    }
    if info > 0 {
        table.add_row(vec![
            Cell::new("Info").fg(TableColor::Blue),
            Cell::new(info).fg(TableColor::Blue),
        ]);
    }

    if result.issues.is_empty() {
        table.add_row(vec![
            Cell::new("No issues found").fg(TableColor::Green),
            Cell::new("✓").fg(TableColor::Green),
        ]);
    }

    println!("{table}");
    println!();

    // Footer
    let footer = format!(
        "  {} files reviewed · {} issues found · model: {}",
        result.files_reviewed,
        result.issues.len(),
        result.model
    );
    if is_tty() {
        println!("{}", footer.dimmed());
    } else {
        println!("{}", footer);
    }
    println!();
}

// ── Main Render Function ───────────────────────────────────

pub fn render_review(result: &ReviewResult, min_severity: Severity) {
    let tw = term_width();

    // Filter and sort issues by severity (highest first)
    let mut issues: Vec<&ReviewIssue> = result
        .issues
        .iter()
        .filter(|i| i.severity >= min_severity)
        .collect();
    issues.sort_by(|a, b| b.severity.cmp(&a.severity));

    if issues.is_empty() && result.issues.is_empty() {
        if is_tty() {
            println!(
                "  {} {}",
                "\u{2705}".green(),
                "No issues found — looking good!".green().bold()
            );
        } else {
            println!("  No issues found — looking good!");
        }
    } else if issues.is_empty() {
        if is_tty() {
            println!(
                "  {} {}",
                "\u{2139}\u{fe0f}",
                format!(
                    "{} issues found but filtered by minimum severity: {}",
                    result.issues.len(),
                    min_severity
                )
                .dimmed()
            );
        }
    }

    for (i, issue) in issues.iter().enumerate() {
        render_issue(issue, i, tw);
    }

    render_summary(result);
}

// ── JSON Output ────────────────────────────────────────────

pub fn render_json(result: &ReviewResult) {
    match serde_json::to_string_pretty(result) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing result: {}", e),
    }
}
