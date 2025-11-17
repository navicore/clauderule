# Quick Start Guide

## Building the Project

```bash
# Build in debug mode (faster compilation)
cargo build

# Build optimized release version
cargo build --release
```

## Running the TUI Viewer

```bash
# Run with debug build
cargo run -- path/to/transcript.jsonl

# Run with release build (faster)
./target/release/claude-log-viewer path/to/transcript.jsonl

# View a Claude Code project directory
cargo run -- ~/.claude/projects/my-project-name
```

## Running the Example

The `basic_parse` example shows how to use the library programmatically:

```bash
cargo run --example basic_parse ~/tmp/git/claude-code-log/test/test_data/representative_messages.jsonl
```

## Project Structure

```
claude-log-viewer/
├── src/
│   ├── main.rs       # CLI binary entry point
│   ├── lib.rs        # Library exports
│   ├── models.rs     # Data structures for JSONL format
│   ├── parser.rs     # JSONL file parsing
│   └── tui.rs        # Ratatui terminal UI
├── examples/
│   └── basic_parse.rs # Example of using the library
├── Cargo.toml        # Project configuration
├── README.md         # Full documentation
└── QUICKSTART.md     # This file
```

## Key Data Structures

### TranscriptEntry

The main enum representing all entry types:

```rust
pub enum TranscriptEntry {
    User(UserTranscriptEntry),          // User messages
    Assistant(AssistantTranscriptEntry), // Claude's responses
    Summary(SummaryTranscriptEntry),     // Session summaries
    System(SystemTranscriptEntry),       // System messages
    QueueOperation(QueueOperationTranscriptEntry), // Internal queue ops
}
```

### ContentItem

Content within messages:

```rust
pub enum ContentItem {
    Text { text: String },                       // Plain text
    ToolUse { id, name, input },                 // Tool invocations
    ToolResult { tool_use_id, content, is_error }, // Tool results
    Thinking { thinking, signature },            // Extended thinking
    Image { source },                            // Base64 images
}
```

## Using as a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
claude-log-viewer = { path = "../claude-log-viewer" }
```

Example code:

```rust
use claude_log_viewer::parser;
use claude_log_viewer::models::TranscriptEntry;

fn main() -> anyhow::Result<()> {
    // Load a JSONL file
    let entries = parser::load_transcript("transcript.jsonl")?;

    // Process entries
    for entry in entries {
        match entry {
            TranscriptEntry::User(u) => {
                println!("User: {}", u.uuid);
            }
            TranscriptEntry::Assistant(a) => {
                println!("Assistant: {} tokens",
                    a.message.usage.map(|u| u.total_tokens()).unwrap_or(0));
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Testing with Sample Data

The Python project includes test data you can use:

```bash
# Single file
cargo run -- ~/tmp/git/claude-code-log/test/test_data/representative_messages.jsonl

# All test files
cargo run -- ~/tmp/git/claude-code-log/test/test_data
```

## TUI Keyboard Controls

- **↑** or **k**: Move up
- **↓** or **j**: Move down
- **q** or **Esc**: Quit

## Viewing Real Claude Code Transcripts

Claude Code stores transcripts at `~/.claude/projects/`:

```bash
# View a specific project
cargo run -- ~/.claude/projects/-Users-you-git-myproject

# Or use the project name shortcut
cargo run -- myproject
```

## Next Steps

1. Try the example: `cargo run --example basic_parse <path>`
2. Read the full README.md for detailed documentation
3. Explore the models.rs file to understand the data structures
4. Build your own tools using the library!

## Troubleshooting

### "Device not configured" error

This error occurs when running the TUI in a non-interactive environment (like CI/CD). The parsing still works - only the TUI display fails.

### Parse errors

If you encounter parse errors, the tool will:
- Skip malformed lines
- Report errors to stderr
- Continue processing valid entries

### Missing data fields

The models use `Option<T>` for optional fields to handle:
- Different JSONL format versions
- Incomplete or legacy data
- Partial transcripts

## Performance

- **Parsing**: ~1-2ms per entry
- **Memory**: Entries are kept in memory (plan ~1KB per entry)
- **Large files**: For 100k+ entries, consider streaming or filtering

## Learn More

- Full documentation: See README.md
- Data models: See src/models.rs
- Parser implementation: See src/parser.rs
- TUI implementation: See src/tui.rs
- Python reference: https://github.com/daaain/claude-code-log
