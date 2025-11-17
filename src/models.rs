//! Rust data models for Claude Code transcript JSONL structures.
//!
//! These models correspond to the Anthropic Claude Code transcript format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main transcript entry enum - all possible entry types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum TranscriptEntry {
    User(UserTranscriptEntry),
    Assistant(AssistantTranscriptEntry),
    Summary(SummaryTranscriptEntry),
    System(SystemTranscriptEntry),
    #[serde(rename = "queue-operation")]
    QueueOperation(QueueOperationTranscriptEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(FileHistorySnapshotEntry),
}

impl TranscriptEntry {
    /// Get the timestamp of this entry if it has one
    pub fn timestamp(&self) -> Option<&str> {
        match self {
            TranscriptEntry::User(e) => Some(&e.timestamp),
            TranscriptEntry::Assistant(e) => Some(&e.timestamp),
            TranscriptEntry::Summary(e) => e.timestamp.as_deref(),
            TranscriptEntry::System(e) => Some(&e.timestamp),
            TranscriptEntry::QueueOperation(e) => Some(&e.timestamp),
            TranscriptEntry::FileHistorySnapshot(e) => e.snapshot.timestamp.as_deref(),
        }
    }

    /// Get the session ID of this entry if it has one
    pub fn session_id(&self) -> Option<&str> {
        match self {
            TranscriptEntry::User(e) => Some(&e.session_id),
            TranscriptEntry::Assistant(e) => Some(&e.session_id),
            TranscriptEntry::Summary(_) => None,
            TranscriptEntry::System(e) => Some(&e.session_id),
            TranscriptEntry::QueueOperation(e) => Some(&e.session_id),
            TranscriptEntry::FileHistorySnapshot(_) => None,
        }
    }

    /// Parse timestamp as DateTime
    pub fn parsed_timestamp(&self) -> Option<DateTime<Utc>> {
        self.timestamp().and_then(|ts| {
            DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
    }
}

/// Base fields common to most transcript entries
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseTranscriptEntry {
    pub parent_uuid: Option<String>,
    pub is_sidechain: bool,
    pub user_type: String,
    pub cwd: String,
    pub session_id: String,
    pub version: String,
    pub uuid: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,
}

/// User message entry
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTranscriptEntry {
    #[serde(flatten)]
    pub base: BaseTranscriptEntry,
    pub message: UserMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<ToolUseResult>,
}

impl std::ops::Deref for UserTranscriptEntry {
    type Target = BaseTranscriptEntry;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/// Assistant message entry
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantTranscriptEntry {
    #[serde(flatten)]
    pub base: BaseTranscriptEntry,
    pub message: AssistantMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl std::ops::Deref for AssistantTranscriptEntry {
    type Target = BaseTranscriptEntry;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/// Summary entry (generated for sessions)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTranscriptEntry {
    pub summary: String,
    pub leaf_uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// System message entry
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTranscriptEntry {
    #[serde(flatten)]
    pub base: BaseTranscriptEntry,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

impl std::ops::Deref for SystemTranscriptEntry {
    type Target = BaseTranscriptEntry;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/// Queue operation entry (enqueue/dequeue tracking)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueOperationTranscriptEntry {
    pub operation: QueueOperation,
    pub timestamp: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentItem>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueOperation {
    Enqueue,
    Dequeue,
}

/// File history snapshot entry (tracks file changes)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySnapshotEntry {
    pub message_id: String,
    pub snapshot: FileHistorySnapshot,
    pub is_snapshot_update: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySnapshot {
    pub message_id: String,
    pub tracked_file_backups: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// User message structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserMessage {
    pub role: String, // Always "user"
    #[serde(deserialize_with = "deserialize_message_content")]
    pub content: MessageContent,
}

/// Assistant message structure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String, // Always "message"
    pub role: String,          // Always "assistant"
    pub model: String,
    pub content: Vec<ContentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageInfo>,
}

/// Message content can be either a string or a list of content items
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Items(Vec<ContentItem>),
}

impl MessageContent {
    /// Extract text from content
    pub fn extract_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Items(items) => items
                .iter()
                .filter_map(|item| match item {
                    ContentItem::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// Custom deserializer for message content
fn deserialize_message_content<'de, D>(deserializer: D) -> Result<MessageContent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::String(s) => Ok(MessageContent::Text(s)),
        serde_json::Value::Array(_) => {
            serde_json::from_value(value)
                .map(MessageContent::Items)
                .map_err(Error::custom)
        }
        _ => Err(Error::custom("Expected string or array for content")),
    }
}

/// Content item - various types of content in messages
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ContentItem {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: HashMap<String, serde_json::Value>,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(deserialize_with = "deserialize_tool_result_content")]
        content: ToolResultContent,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    Image {
        source: ImageSource,
    },
}

/// Tool result content can be string or list of content items
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Items(Vec<ContentItem>),
}

/// Custom deserializer for tool result content
fn deserialize_tool_result_content<'de, D>(deserializer: D) -> Result<ToolResultContent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::String(s) => Ok(ToolResultContent::Text(s)),
        serde_json::Value::Array(_) => {
            serde_json::from_value(value)
                .map(ToolResultContent::Items)
                .map_err(Error::custom)
        }
        _ => Err(Error::custom("Expected string or array for tool result content")),
    }
}

/// Image source structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // Always "base64"
    pub media_type: String,
    pub data: String,
}

/// Token usage information
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UsageInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

impl UsageInfo {
    /// Calculate total tokens used
    pub fn total_tokens(&self) -> i32 {
        self.input_tokens.unwrap_or(0)
            + self.cache_creation_input_tokens.unwrap_or(0)
            + self.output_tokens.unwrap_or(0)
    }

    /// Calculate cost-effective tokens (with cache reads)
    pub fn effective_input_tokens(&self) -> i32 {
        self.input_tokens.unwrap_or(0)
            + self.cache_creation_input_tokens.unwrap_or(0)
            + self.cache_read_input_tokens.unwrap_or(0)
    }
}

/// Tool use result - can be various types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolUseResult {
    Text(String),
    FileRead(FileReadResult),
    Command(CommandResult),
    Todo(TodoResult),
    Edit(EditResult),
    Todos(Vec<TodoItem>),
    ContentItems(Vec<ContentItem>),
}

/// File read result
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileReadResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub file: FileInfo,
}

/// File information
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub file_path: String,
    pub content: String,
    pub num_lines: i32,
    pub start_line: i32,
    pub total_lines: i32,
}

/// Command execution result
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub interrupted: bool,
    pub is_image: bool,
}

/// Todo item
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

/// Todo result (old and new todos)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoResult {
    pub old_todos: Vec<TodoItem>,
    pub new_todos: Vec<TodoItem>,
}

/// Edit result
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace_all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_modified: Option<bool>,
}
