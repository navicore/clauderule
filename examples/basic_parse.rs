//! Basic example of parsing a Claude Code JSONL file without the TUI.
//!
//! Run with: cargo run --example basic_parse path/to/transcript.jsonl

use claude_log_viewer::{models::TranscriptEntry, parser};
use std::env;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run --example basic_parse <path-to-jsonl>");
        std::process::exit(1);
    }

    let path = &args[1];
    println!("Loading transcript from: {}", path);

    // Load and parse the JSONL file
    let entries = parser::load_transcript(path)?;
    println!("Loaded {} entries\n", entries.len());

    // Iterate through entries and print basic info
    for (i, entry) in entries.iter().enumerate() {
        match entry {
            TranscriptEntry::User(u) => {
                println!("[{}] USER ({})", i + 1, u.timestamp);
                println!("    Session: {}", u.session_id);
                println!("    UUID: {}", u.uuid);

                // Print first 100 chars of content
                let content_str = match &u.message.content {
                    claude_log_viewer::models::MessageContent::Text(s) => s.clone(),
                    claude_log_viewer::models::MessageContent::Items(items) => {
                        format!("{} content items", items.len())
                    }
                };
                let preview = if content_str.len() > 100 {
                    format!("{}...", &content_str[..100])
                } else {
                    content_str
                };
                println!("    Content: {}\n", preview);
            }
            TranscriptEntry::Assistant(a) => {
                println!("[{}] ASSISTANT ({})", i + 1, a.timestamp);
                println!("    Model: {}", a.message.model);
                println!("    Content blocks: {}", a.message.content.len());

                if let Some(usage) = &a.message.usage {
                    println!("    Tokens: {} total", usage.total_tokens());
                    if let Some(input) = usage.input_tokens {
                        println!("      - Input: {}", input);
                    }
                    if let Some(output) = usage.output_tokens {
                        println!("      - Output: {}", output);
                    }
                }
                println!();
            }
            TranscriptEntry::Summary(s) => {
                println!("[{}] SUMMARY", i + 1);
                println!("    {}\n", s.summary);
            }
            TranscriptEntry::System(s) => {
                println!("[{}] SYSTEM ({})", i + 1, s.timestamp);
                println!("    {}\n", s.content);
            }
            TranscriptEntry::QueueOperation(q) => {
                println!("[{}] QUEUE {:?}", i + 1, q.operation);
            }
        }
    }

    // Print summary statistics
    let mut total_tokens = 0;
    let mut assistant_count = 0;

    for entry in &entries {
        if let TranscriptEntry::Assistant(a) = entry {
            assistant_count += 1;
            if let Some(usage) = &a.message.usage {
                total_tokens += usage.total_tokens();
            }
        }
    }

    println!("=== Statistics ===");
    println!("Total entries: {}", entries.len());
    println!("Total tokens used: {}", total_tokens);
    if assistant_count > 0 {
        println!(
            "Average tokens per assistant message: {}",
            total_tokens / assistant_count
        );
    }

    Ok(())
}
