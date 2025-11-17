//! Analytics for Claude Code transcripts - file access tracking, phrase analysis, etc.

use crate::models::{AssistantTranscriptEntry, ContentItem, TranscriptEntry, UserTranscriptEntry};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Track file access operations from transcripts
#[derive(Debug, Default)]
pub struct FileAccessTracker {
    /// File path -> count of accesses
    pub read_counts: HashMap<PathBuf, usize>,
    pub write_counts: HashMap<PathBuf, usize>,
    pub edit_counts: HashMap<PathBuf, usize>,
}

impl FileAccessTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a single transcript entry for file operations
    pub fn analyze_entry(&mut self, entry: &TranscriptEntry) {
        match entry {
            TranscriptEntry::Assistant(a) => {
                self.analyze_assistant_message(a);
            }
            TranscriptEntry::User(u) => {
                self.analyze_user_message(u);
            }
            _ => {}
        }
    }

    fn analyze_assistant_message(&mut self, msg: &AssistantTranscriptEntry) {
        for item in &msg.message.content {
            if let ContentItem::ToolUse { name, input, .. } = item {
                self.track_tool_use(name, input);
            }
        }
    }

    fn analyze_user_message(&mut self, msg: &UserTranscriptEntry) {
        // User messages can contain tool results, but we're tracking tool uses
        // which appear in assistant messages
    }

    fn track_tool_use(&mut self, tool_name: &str, input: &HashMap<String, serde_json::Value>) {
        // Extract file_path from tool input
        let file_path = match input.get("file_path") {
            Some(serde_json::Value::String(path)) => PathBuf::from(path),
            _ => return,
        };

        // Track based on tool name
        match tool_name {
            "Read" => {
                *self.read_counts.entry(file_path).or_insert(0) += 1;
            }
            "Write" => {
                *self.write_counts.entry(file_path).or_insert(0) += 1;
            }
            "Edit" | "MultiEdit" => {
                *self.edit_counts.entry(file_path).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    /// Get all files matching a pattern, sorted by access count
    pub fn files_matching(&self, pattern: &str) -> Vec<(PathBuf, usize)> {
        let mut results = Vec::new();

        // Combine all file accesses
        let mut all_files: HashMap<PathBuf, usize> = HashMap::new();
        for (path, count) in &self.read_counts {
            *all_files.entry(path.clone()).or_insert(0) += count;
        }
        for (path, count) in &self.write_counts {
            *all_files.entry(path.clone()).or_insert(0) += count;
        }
        for (path, count) in &self.edit_counts {
            *all_files.entry(path.clone()).or_insert(0) += count;
        }

        // Filter by pattern
        for (path, count) in all_files {
            if matches_pattern(path.to_str().unwrap_or(""), pattern) {
                results.push((path, count));
            }
        }

        // Sort by count descending
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }

    /// Get top N most accessed files overall
    pub fn top_files(&self, n: usize) -> Vec<(PathBuf, FileAccessStats)> {
        let mut all_files: HashMap<PathBuf, FileAccessStats> = HashMap::new();

        for (path, count) in &self.read_counts {
            all_files.entry(path.clone()).or_default().reads = *count;
        }
        for (path, count) in &self.write_counts {
            all_files.entry(path.clone()).or_default().writes = *count;
        }
        for (path, count) in &self.edit_counts {
            all_files.entry(path.clone()).or_default().edits = *count;
        }

        let mut results: Vec<_> = all_files.into_iter().collect();
        results.sort_by(|a, b| b.1.total().cmp(&a.1.total()));
        results.into_iter().take(n).collect()
    }

    /// Get statistics
    pub fn stats(&self) -> TrackerStats {
        TrackerStats {
            total_reads: self.read_counts.values().sum(),
            total_writes: self.write_counts.values().sum(),
            total_edits: self.edit_counts.values().sum(),
            unique_files_read: self.read_counts.len(),
            unique_files_written: self.write_counts.len(),
            unique_files_edited: self.edit_counts.len(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct FileAccessStats {
    pub reads: usize,
    pub writes: usize,
    pub edits: usize,
}

impl FileAccessStats {
    pub fn total(&self) -> usize {
        self.reads + self.writes + self.edits
    }
}

#[derive(Debug)]
pub struct TrackerStats {
    pub total_reads: usize,
    pub total_writes: usize,
    pub total_edits: usize,
    pub unique_files_read: usize,
    pub unique_files_written: usize,
    pub unique_files_edited: usize,
}

/// Simple glob-like pattern matching
fn matches_pattern(path: &str, pattern: &str) -> bool {
    // Match everything
    if pattern == "*" {
        return true;
    }

    // Handle some common patterns
    if pattern.contains('*') {
        // Convert simple glob to regex-like matching
        if pattern.starts_with('*') && pattern.ends_with('*') && pattern.len() > 2 {
            // *foo* - contains
            let inner = &pattern[1..pattern.len() - 1];
            path.contains(inner)
        } else if pattern.starts_with('*') {
            // *foo - ends with
            let suffix = &pattern[1..];
            path.ends_with(suffix)
        } else if pattern.ends_with('*') {
            // foo* - starts with
            let prefix = &pattern[..pattern.len() - 1];
            path.starts_with(prefix)
        } else {
            // foo*bar - more complex, use basic matching
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                path.starts_with(parts[0]) && path.ends_with(parts[1])
            } else {
                // Complex pattern - just check if it contains all parts
                parts.iter().all(|part| path.contains(part))
            }
        }
    } else {
        // Exact match or substring
        path.contains(pattern)
    }
}

/// Context around a phrase occurrence
#[derive(Debug, Clone)]
pub struct PhraseContext {
    pub phrase: String,
    pub context: String,
    pub timestamp: String,
}

/// Track user phrase usage
#[derive(Debug, Default)]
pub struct PhraseTracker {
    /// Phrase -> list of contexts where it appeared
    pub phrase_contexts: HashMap<String, Vec<PhraseContext>>,
    /// Whether to use regex matching
    pub use_regex: bool,
}

impl PhraseTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new tracker with regex support
    pub fn with_regex() -> Self {
        Self {
            phrase_contexts: HashMap::new(),
            use_regex: true,
        }
    }

    /// Analyze entries for phrase usage
    pub fn analyze_entry(&mut self, entry: &TranscriptEntry, phrases: &[String]) {
        if self.use_regex {
            self.analyze_entry_regex(entry, phrases);
        } else {
            self.analyze_entry_literal(entry, phrases);
        }
    }

    /// Analyze using literal substring matching
    fn analyze_entry_literal(&mut self, entry: &TranscriptEntry, phrases: &[String]) {
        if let TranscriptEntry::User(u) = entry {
            let content = match &u.message.content {
                crate::models::MessageContent::Text(s) => s.clone(),
                crate::models::MessageContent::Items(items) => {
                    items
                        .iter()
                        .filter_map(|item| {
                            if let ContentItem::Text { text } = item {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };

            let content_lower = content.to_lowercase();

            for phrase in phrases {
                let phrase_lower = phrase.to_lowercase();

                // Find all occurrences and extract context
                let mut start = 0;
                while let Some(pos) = content_lower[start..].find(&phrase_lower) {
                    let abs_pos = start + pos;

                    // Extract context (±100 chars or to sentence boundaries)
                    let context = extract_context(&content, abs_pos, phrase.len());

                    self.phrase_contexts
                        .entry(phrase.clone())
                        .or_insert_with(Vec::new)
                        .push(PhraseContext {
                            phrase: phrase.clone(),
                            context,
                            timestamp: u.timestamp.clone(),
                        });

                    start = abs_pos + phrase.len();
                }
            }
        }
    }

    /// Analyze using regex pattern matching
    fn analyze_entry_regex(&mut self, entry: &TranscriptEntry, patterns: &[String]) {
        if let TranscriptEntry::User(u) = entry {
            let content = match &u.message.content {
                crate::models::MessageContent::Text(s) => s.clone(),
                crate::models::MessageContent::Items(items) => {
                    items
                        .iter()
                        .filter_map(|item| {
                            if let ContentItem::Text { text } = item {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };

            for pattern in patterns {
                // Compile regex with case-insensitive flag
                let re = match Regex::new(&format!("(?i){}", pattern)) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Invalid regex pattern '{}': {}", pattern, e);
                        continue;
                    }
                };

                // Find all matches
                for mat in re.find_iter(&content) {
                    let pos = mat.start();
                    let matched_text = mat.as_str();

                    // Extract context around the match
                    let context = extract_context(&content, pos, matched_text.len());

                    self.phrase_contexts
                        .entry(pattern.clone())
                        .or_insert_with(Vec::new)
                        .push(PhraseContext {
                            phrase: matched_text.to_string(), // Store actual matched text
                            context,
                            timestamp: u.timestamp.clone(),
                        });
                }
            }
        }
    }

    /// Get phrases with their contexts
    pub fn get_contexts(&self, phrase: &str) -> Vec<&PhraseContext> {
        self.phrase_contexts
            .get(phrase)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Get summary: phrase -> occurrence count
    pub fn get_summary(&self) -> Vec<(String, usize)> {
        let mut results: Vec<_> = self.phrase_contexts
            .iter()
            .map(|(phrase, contexts)| (phrase.clone(), contexts.len()))
            .collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }
}

/// Extract context around a phrase occurrence
fn extract_context(text: &str, pos: usize, phrase_len: usize) -> String {
    const CONTEXT_CHARS: usize = 100;

    // Find start position (go back up to CONTEXT_CHARS, but stop at sentence boundary)
    let start = if pos > CONTEXT_CHARS {
        // Try to find sentence start (. ! ?)
        let search_start = pos.saturating_sub(CONTEXT_CHARS);
        text[search_start..pos]
            .rfind(|c| c == '.' || c == '!' || c == '?')
            .map(|p| search_start + p + 1)
            .unwrap_or(search_start)
    } else {
        0
    };

    // Find end position
    let end_pos = pos + phrase_len;
    let end = if end_pos + CONTEXT_CHARS < text.len() {
        // Try to find sentence end
        let search_end = end_pos + CONTEXT_CHARS;
        text[end_pos..search_end]
            .find(|c| c == '.' || c == '!' || c == '?')
            .map(|p| end_pos + p + 1)
            .unwrap_or(search_end)
    } else {
        text.len()
    };

    let mut context = text[start..end].trim().to_string();

    // Add ellipsis if we truncated
    if start > 0 {
        context = format!("...{}", context);
    }
    if end < text.len() {
        context = format!("{}...", context);
    }

    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("/path/to/.env", ".env"));
        assert!(matches_pattern("/path/to/.env", "*.env"));
        assert!(matches_pattern("/path/to/.env", ".env*"));
        assert!(matches_pattern("/path/to/.env", "*/.env"));
        assert!(!matches_pattern("/path/to/file.rs", ".env"));
    }

    #[test]
    fn test_glob_patterns() {
        assert!(matches_pattern("/Users/foo/.aws/config", "*.aws/*"));
        assert!(matches_pattern("/Users/foo/.aws/config", "~/.aws/*"));
        assert!(matches_pattern("/path/to/.zshrc", "*.zshrc"));
    }
}
