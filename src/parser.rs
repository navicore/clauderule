//! Parser for Claude Code transcript JSONL files.

use crate::models::TranscriptEntry;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Load and parse a single JSONL transcript file
pub fn load_transcript<P: AsRef<Path>>(path: P) -> Result<Vec<TranscriptEntry>> {
    let path = path.as_ref();
    let file = File::open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;

    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    let mut errors = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.with_context(|| {
            format!("Failed to read line {} from {}", line_num + 1, path.display())
        })?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Try to parse the line
        match serde_json::from_str::<TranscriptEntry>(&line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                errors.push(format!(
                    "Line {}: Parse error: {}",
                    line_num + 1,
                    e
                ));
            }
        }
    }

    // Print errors to stderr but don't fail
    if !errors.is_empty() {
        eprintln!("Encountered {} parse errors in {}:", errors.len(), path.display());
        for error in errors.iter().take(10) {
            eprintln!("  {}", error);
        }
        if errors.len() > 10 {
            eprintln!("  ... and {} more errors", errors.len() - 10);
        }
    }

    // Sort entries by timestamp
    entries.sort_by(|a, b| {
        match (a.timestamp(), b.timestamp()) {
            (Some(ts_a), Some(ts_b)) => ts_a.cmp(ts_b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    Ok(entries)
}

/// Load all JSONL files from a directory
pub fn load_directory_transcripts<P: AsRef<Path>>(dir_path: P) -> Result<Vec<TranscriptEntry>> {
    let dir_path = dir_path.as_ref();
    let mut all_entries = Vec::new();

    let entries = std::fs::read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Only process .jsonl files
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            println!("Loading {}...", path.display());
            let transcript = load_transcript(&path)?;
            all_entries.extend(transcript);
        }
    }

    // Sort all entries by timestamp
    all_entries.sort_by(|a, b| {
        match (a.timestamp(), b.timestamp()) {
            (Some(ts_a), Some(ts_b)) => ts_a.cmp(ts_b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    Ok(all_entries)
}

/// Find the Claude Code projects directory
pub fn find_claude_projects_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("projects"))
}

/// List all project directories in ~/.claude/projects/
pub fn list_projects() -> Result<Vec<std::path::PathBuf>> {
    let projects_dir = find_claude_projects_dir()
        .context("Could not find home directory")?;

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();
    for entry in std::fs::read_dir(&projects_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            projects.push(path);
        }
    }

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_example() {
        // This would test parsing a sample JSONL entry
        let json = r#"{"type":"user","timestamp":"2025-07-03T15:50:07.874907Z","parentUuid":null,"isSidechain":false,"userType":"human","cwd":"/tmp","sessionId":"test_session","version":"1.0.0","uuid":"msg_001","message":{"role":"user","content":[{"type":"text","text":"Hello Claude!"}]}}"#;

        let entry: TranscriptEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(entry, TranscriptEntry::User(_)));
    }
}
