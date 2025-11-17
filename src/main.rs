mod analytics;
mod models;
mod parser;
mod project_finder;
mod settings;
mod tui;

use anyhow::Result;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    // Check for subcommands
    match args[1].as_str() {
        "settings" => {
            return handle_settings_command(&args[2..]);
        }
        "files" => {
            return handle_files_command(&args[2..]);
        }
        "phrases" => {
            return handle_phrases_command(&args[2..]);
        }
        "view" => {
            if args.len() < 3 {
                eprintln!("Error: 'view' command requires a path");
                return Ok(());
            }
            return handle_view_command(&args[2]);
        }
        // Default: treat first arg as path for backward compatibility
        _ => {
            return handle_view_command(&args[1]);
        }
    }
}

fn handle_settings_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Usage: claude-log-viewer settings <subcommand>");
        eprintln!();
        eprintln!("Subcommands:");
        eprintln!("  aggregate [path]    Aggregate settings from all projects");
        eprintln!("  find <pattern>      Find specific permission patterns");
        return Ok(());
    }

    match args[0].as_str() {
        "aggregate" => {
            let base_dir = if args.len() > 1 {
                PathBuf::from(&args[1])
            } else {
                // Default to current directory or search for .claude directories
                std::env::current_dir()?
            };

            println!("Aggregating settings from: {}", base_dir.display());
            let agg = settings::aggregate_settings(&base_dir)?;

            println!("\n=== Settings Aggregation Report ===\n");
            println!("Total projects analyzed: {}", agg.total_projects());
            println!("Projects with MCP enabled: {}", agg.mcp_enabled_projects.len());

            println!("\n--- Most Common ALLOW Permissions ---");
            for (perm, count) in agg.top_allowed(10) {
                println!("  {:40} ({} projects)", perm.to_string(), count);
            }

            if !agg.deny_map.is_empty() {
                println!("\n--- Most Common DENY Permissions ---");
                for (perm, count) in agg.top_denied(10) {
                    println!("  {:40} ({} projects)", perm.to_string(), count);
                }
            }

            // Security insights
            println!("\n--- Security Insights ---");
            let websearch_count = agg.allow_map.get(&settings::Permission::WebSearch)
                .map(|v| v.len())
                .unwrap_or(0);
            if websearch_count > 0 {
                println!("  ⚠️  {} projects allow unrestricted WebSearch", websearch_count);
            }

            let bash_any = agg.allow_map.iter()
                .filter(|(p, _)| matches!(p, settings::Permission::Bash(pat) if pat.contains('*')))
                .count();
            if bash_any > 0 {
                println!("  ⚠️  {} wildcard Bash permissions found", bash_any);
            }

            if agg.deny_map.is_empty() {
                println!("  ℹ️  No projects use deny rules");
            }
        }
        "find" => {
            if args.len() < 2 {
                eprintln!("Usage: claude-log-viewer settings find <pattern>");
                return Ok(());
            }
            let pattern = &args[1];
            println!("Finding permissions matching: {}", pattern);
            // TODO: Implement pattern search
        }
        _ => {
            eprintln!("Unknown settings subcommand: {}", args[0]);
        }
    }

    Ok(())
}

fn handle_files_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Usage: claude-log-viewer files <pattern> [path]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  claude-log-viewer files '.env' ~/git/myproject");
        eprintln!("  claude-log-viewer files '*.zshrc' .");
        eprintln!("  claude-log-viewer files '~/.aws/*'  # uses current directory");
        return Ok(());
    }

    let pattern = &args[0];
    let user_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir()?
    };

    println!("Analyzing file access for pattern: {}", pattern);
    println!("Project path: {}", user_path.display());

    // Load all transcripts - handle direct .jsonl files or project directories
    let entries = if user_path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
        // Direct JSONL file - use it directly
        parser::load_transcript(&user_path)?
    } else {
        // Project directory - find Claude transcripts
        let claude_path = project_finder::find_claude_project_dir(&user_path)?;
        println!("Claude transcripts: {}\n", claude_path.display());

        if claude_path.is_file() {
            parser::load_transcript(&claude_path)?
        } else {
            parser::load_directory_transcripts(&claude_path)?
        }
    };

    // Track file access
    let mut tracker = analytics::FileAccessTracker::new();
    for entry in &entries {
        tracker.analyze_entry(entry);
    }

    // Get files matching pattern
    let matches = tracker.files_matching(pattern);

    println!("=== File Access Report ===\n");
    if matches.is_empty() {
        println!("No files matching '{}' were accessed", pattern);
    } else {
        println!("Files matching '{}' (sorted by access count):\n", pattern);
        for (file_path, count) in matches {
            println!("  {:60} {:>4} accesses", file_path.display(), count);
        }
    }

    // Print overall statistics
    let stats = tracker.stats();
    println!("\n--- Overall Statistics ---");
    println!("Total file reads:  {}", stats.total_reads);
    println!("Total file writes: {}", stats.total_writes);
    println!("Total file edits:  {}", stats.total_edits);
    println!("Unique files read: {}", stats.unique_files_read);

    // Show top 10 most accessed files
    println!("\n--- Top 10 Most Accessed Files ---");
    for (file_path, stats) in tracker.top_files(10) {
        println!(
            "  {:50} R:{:>3} W:{:>3} E:{:>3} Total:{}",
            file_path.display(),
            stats.reads,
            stats.writes,
            stats.edits,
            stats.total()
        );
    }

    Ok(())
}

fn handle_phrases_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Usage: claude-log-viewer phrases [--regex] [path] <phrase1> [phrase2] ...");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --regex    Use regex pattern matching instead of literal strings");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  # Literal phrase matching");
        eprintln!("  claude-log-viewer phrases ~/git/myproject 'how do I' 'can you help'");
        eprintln!("  claude-log-viewer phrases 'not working' 'try again'  # uses current dir");
        eprintln!();
        eprintln!("  # Regex pattern matching");
        eprintln!("  claude-log-viewer phrases --regex 'never (ever|again)'");
        eprintln!("  claude-log-viewer phrases --regex 'how do (I|you|we)'");
        eprintln!("  claude-log-viewer phrases --regex '(can|could) you help'");
        return Ok(());
    }

    // Check for --regex flag
    let use_regex = args.first().map(|s| s.as_str()) == Some("--regex");
    let args = if use_regex { &args[1..] } else { args };

    if args.is_empty() {
        eprintln!("Error: No patterns provided");
        return Ok(());
    }

    // Determine if first arg is a path or a phrase
    let (user_path, phrases): (PathBuf, Vec<String>) = if args.len() > 1 && PathBuf::from(&args[0]).exists() {
        // First arg is a path
        (
            PathBuf::from(&args[0]),
            args[1..].iter().map(|s| s.to_string()).collect(),
        )
    } else {
        // No path provided, use current directory
        (
            std::env::current_dir()?,
            args.iter().map(|s| s.to_string()).collect(),
        )
    };

    println!("Project path: {}", user_path.display());
    if use_regex {
        println!("Using regex pattern matching");
    }
    println!("Searching for {} patterns\n", phrases.len());

    // Load transcripts - handle direct .jsonl files or project directories
    let entries = if user_path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
        // Direct JSONL file - use it directly
        parser::load_transcript(&user_path)?
    } else {
        // Project directory - find Claude transcripts
        let claude_path = project_finder::find_claude_project_dir(&user_path)?;
        println!("Claude transcripts: {}", claude_path.display());

        if claude_path.is_file() {
            parser::load_transcript(&claude_path)?
        } else {
            parser::load_directory_transcripts(&claude_path)?
        }
    };

    // Track phrases with optional regex
    let mut tracker = if use_regex {
        analytics::PhraseTracker::with_regex()
    } else {
        analytics::PhraseTracker::new()
    };

    for entry in &entries {
        tracker.analyze_entry(entry, &phrases);
    }

    // Print results
    println!("\n=== Phrase Usage Report ===\n");
    let summary = tracker.get_summary();

    if summary.is_empty() {
        println!("None of the specified phrases were found");
    } else {
        for (phrase, count) in &summary {
            println!("Phrase: \"{}\" ({} occurrences)\n", phrase, count);

            // Show each context
            let contexts = tracker.get_contexts(phrase);
            for (i, ctx) in contexts.iter().enumerate() {
                let time_short = ctx.timestamp
                    .split('T')
                    .nth(1)
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap_or("");

                println!("  [{}] {} - {}", i + 1, time_short, ctx.context);
            }
            println!();
        }
    }

    Ok(())
}

fn handle_view_command(path_str: &str) -> Result<()> {
    let path = PathBuf::from(path_str);

    let entries = if path.is_file() {
        // Load single file
        println!("Loading transcript from file: {}", path.display());
        parser::load_transcript(&path)?
    } else if path.is_dir() {
        // Load all JSONL files from directory
        println!("Loading transcripts from directory: {}", path.display());
        parser::load_directory_transcripts(&path)?
    } else {
        // Try to find in Claude projects directory
        if let Some(projects_dir) = parser::find_claude_projects_dir() {
            let project_path = projects_dir.join(path_str);
            if project_path.is_dir() {
                println!("Loading transcripts from project: {}", project_path.display());
                parser::load_directory_transcripts(&project_path)?
            } else {
                anyhow::bail!("Path does not exist: {}", path.display());
            }
        } else {
            anyhow::bail!("Path does not exist: {}", path.display());
        }
    };

    println!("Loaded {} transcript entries", entries.len());

    // Print summary statistics
    print_summary(&entries);

    // Launch TUI
    println!("\nLaunching TUI...\n");
    tui::run_tui(entries)?;

    Ok(())
}

fn print_usage() {
    println!("Claude Code Analytics");
    println!();
    println!("USAGE:");
    println!("  claude-log-viewer <command> [args]");
    println!();
    println!("COMMANDS:");
    println!("  view <path>                      View transcripts in TUI (default)");
    println!("  settings aggregate [path]        Aggregate permission settings");
    println!("  files <pattern> [path]           Track file access by pattern");
    println!("  phrases [--regex] [path] <p>...  Track user phrase usage");
    println!();
    println!("EXAMPLES:");
    println!("  # Analyze settings from source directory tree");
    println!("  claude-log-viewer settings aggregate ~/git/navicore");
    println!();
    println!("  # Find .env file accesses (automatically finds Claude transcripts)");
    println!("  claude-log-viewer files '.env' ~/git/myproject");
    println!("  claude-log-viewer files '.env'  # uses current directory");
    println!();
    println!("  # Find ~/.zshrc or ~/.aws/* file accesses");
    println!("  claude-log-viewer files '*.zshrc' ~/git/myproject");
    println!("  claude-log-viewer files '.aws' ~/git/myproject");
    println!();
    println!("  # Track common user phrases (literal matching)");
    println!("  claude-log-viewer phrases ~/git/project 'how do I' 'can you help'");
    println!("  claude-log-viewer phrases 'not working' 'error'  # uses current dir");
    println!();
    println!("  # Track phrases with regex patterns");
    println!("  claude-log-viewer phrases --regex 'never (ever|again)'");
    println!("  claude-log-viewer phrases --regex '(can|could) you help'");
    println!();
    println!("NOTE:");
    println!("  For files/phrases commands, point to your project source directory.");
    println!("  The tool automatically finds the corresponding ~/.claude/projects/ data.");
    println!();
    println!("KEYBOARD SHORTCUTS (TUI):");
    println!("  ↑/k       Move up");
    println!("  ↓/j       Move down");
    println!("  q/Esc     Quit");
}

fn print_summary(entries: &[models::TranscriptEntry]) {
    let mut user_count = 0;
    let mut assistant_count = 0;
    let mut summary_count = 0;
    let mut system_count = 0;
    let mut queue_count = 0;
    let mut total_tokens = 0;

    for entry in entries {
        match entry {
            models::TranscriptEntry::User(_) => user_count += 1,
            models::TranscriptEntry::Assistant(a) => {
                assistant_count += 1;
                if let Some(usage) = &a.message.usage {
                    total_tokens += usage.total_tokens();
                }
            }
            models::TranscriptEntry::Summary(_) => summary_count += 1,
            models::TranscriptEntry::System(_) => system_count += 1,
            models::TranscriptEntry::QueueOperation(_) => queue_count += 1,
            models::TranscriptEntry::FileHistorySnapshot(_) => {} // Skip in summary
        }
    }

    println!("\n=== Summary ===");
    println!("User messages:      {}", user_count);
    println!("Assistant messages: {}", assistant_count);
    println!("Summaries:          {}", summary_count);
    println!("System messages:    {}", system_count);
    println!("Queue operations:   {}", queue_count);
    println!("Total tokens used:  {}", total_tokens);

    if assistant_count > 0 {
        println!(
            "Avg tokens/message: {}",
            total_tokens / assistant_count as i32
        );
    }
}
