//! Terminal User Interface for viewing Claude Code transcripts.

use crate::models::{ContentItem, MessageContent, TranscriptEntry, UsageInfo};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

pub struct App {
    entries: Vec<TranscriptEntry>,
    list_state: ListState,
    should_quit: bool,
}

impl App {
    pub fn new(entries: Vec<TranscriptEntry>) -> Self {
        let mut list_state = ListState::default();
        if !entries.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            entries,
            list_state,
            should_quit: false,
        }
    }

    pub fn next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.entries.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn selected_entry(&self) -> Option<&TranscriptEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.entries.get(i))
    }
}

pub fn run_tui(entries: Vec<TranscriptEntry>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(entries);
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => {
                    app.should_quit = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.next();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.previous();
                }
                KeyCode::Esc => {
                    app.should_quit = true;
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(f.area());

    // Create list items
    let items: Vec<ListItem> = app
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let (icon, timestamp, summary) = match entry {
                TranscriptEntry::User(u) => {
                    let text = match &u.message.content {
                        MessageContent::Text(s) => s.chars().take(60).collect::<String>(),
                        MessageContent::Items(items) => items
                            .iter()
                            .filter_map(|item| match item {
                                ContentItem::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                            .chars()
                            .take(60)
                            .collect::<String>(),
                    };
                    ("👤", u.timestamp.clone(), text)
                }
                TranscriptEntry::Assistant(a) => {
                    let text = a
                        .message
                        .content
                        .iter()
                        .filter_map(|item| match item {
                            ContentItem::Text { text } => Some(text.as_str()),
                            ContentItem::ToolUse { name, .. } => Some(name.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                        .chars()
                        .take(60)
                        .collect::<String>();

                    let tokens = a
                        .message
                        .usage
                        .as_ref()
                        .map(|u| format!(" [{}t]", u.total_tokens()))
                        .unwrap_or_default();

                    ("🤖", a.timestamp.clone(), format!("{}{}", text, tokens))
                }
                TranscriptEntry::Summary(s) => {
                    let text = s.summary.chars().take(60).collect::<String>();
                    ("📋", s.timestamp.clone().unwrap_or_default(), text)
                }
                TranscriptEntry::System(s) => {
                    let text = s.content.chars().take(60).collect::<String>();
                    ("⚙️", s.timestamp.clone(), text)
                }
                TranscriptEntry::QueueOperation(q) => (
                    "📥",
                    q.timestamp.clone(),
                    format!("{:?}", q.operation),
                ),
                TranscriptEntry::FileHistorySnapshot(f) => (
                    "📸",
                    f.snapshot.timestamp.clone().unwrap_or_default(),
                    "File snapshot".to_string(),
                ),
            };

            let time_short = timestamp
                .split('T')
                .nth(1)
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("");

            let content = Line::from(vec![
                Span::raw(format!("{:>3} ", idx + 1)),
                Span::raw(format!("{} ", icon)),
                Span::styled(
                    format!("{:>8} ", time_short),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(summary),
            ]);

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Claude Code Transcript (↑/↓ or j/k to navigate, q to quit)")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // Detail view
    let detail = if let Some(entry) = app.selected_entry() {
        format_entry_detail(entry)
    } else {
        vec![Line::from("No entry selected")]
    };

    let paragraph = Paragraph::new(detail)
        .block(Block::default().title("Detail").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, chunks[1]);
}

fn format_entry_detail(entry: &TranscriptEntry) -> Vec<Line<'_>> {
    let mut lines = Vec::new();

    match entry {
        TranscriptEntry::User(u) => {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("User"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Session: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&u.session_id),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Timestamp: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&u.timestamp),
            ]));
            lines.push(Line::from(""));

            match &u.message.content {
                MessageContent::Text(text) => {
                    lines.push(Line::from(Span::styled(
                        "Content:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )));
                    for line in text.lines() {
                        lines.push(Line::from(line.to_string()));
                    }
                }
                MessageContent::Items(items) => {
                    for item in items {
                        format_content_item(item, &mut lines);
                    }
                }
            }

            if let Some(result) = &u.tool_use_result {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Tool Result:",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(format!("{:?}", result)));
            }
        }
        TranscriptEntry::Assistant(a) => {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Assistant"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Model: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&a.message.model),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Session: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&a.session_id),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Timestamp: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&a.timestamp),
            ]));

            if let Some(usage) = &a.message.usage {
                format_usage_info(usage, &mut lines);
            }

            lines.push(Line::from(""));

            for item in &a.message.content {
                format_content_item(item, &mut lines);
            }
        }
        TranscriptEntry::Summary(s) => {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Summary"),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Summary:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for line in s.summary.lines() {
                lines.push(Line::from(line.to_string()));
            }
        }
        TranscriptEntry::System(s) => {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("System"),
            ]));
            if let Some(level) = &s.level {
                lines.push(Line::from(vec![
                    Span::styled("Level: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(level),
                ]));
            }
            lines.push(Line::from(""));
            for line in s.content.lines() {
                lines.push(Line::from(line.to_string()));
            }
        }
        TranscriptEntry::QueueOperation(q) => {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Queue Operation"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Operation: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{:?}", q.operation)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Timestamp: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&q.timestamp),
            ]));
        }
        TranscriptEntry::FileHistorySnapshot(f) => {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("File History Snapshot"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Message ID: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&f.message_id),
            ]));
            if let Some(ts) = &f.snapshot.timestamp {
                lines.push(Line::from(vec![
                    Span::styled("Timestamp: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(ts),
                ]));
            }
        }
    }

    lines
}

fn format_content_item(item: &ContentItem, lines: &mut Vec<Line>) {
    match item {
        ContentItem::Text { text } => {
            lines.push(Line::from(Span::styled(
                "Text:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for line in text.lines() {
                lines.push(Line::from(line.to_string()));
            }
            lines.push(Line::from(""));
        }
        ContentItem::ToolUse { id, name, input } => {
            lines.push(Line::from(Span::styled(
                format!("Tool Use: {}", name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::raw("  ID: "),
                Span::styled(id.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from("  Input:"));
            for (key, value) in input {
                let value_str = value.to_string();
                let truncated = if value_str.len() > 100 {
                    format!("{}...", &value_str[..100])
                } else {
                    value_str
                };
                lines.push(Line::from(format!("    {}: {}", key, truncated)));
            }
            lines.push(Line::from(""));
        }
        ContentItem::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let error_marker = if is_error.unwrap_or(false) {
                " [ERROR]"
            } else {
                ""
            };
            lines.push(Line::from(Span::styled(
                format!("Tool Result{}", error_marker),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::raw("  Tool Use ID: "),
                Span::styled(tool_use_id.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(format!("  Content: {:?}", content)));
            lines.push(Line::from(""));
        }
        ContentItem::Thinking { thinking, .. } => {
            lines.push(Line::from(Span::styled(
                "Thinking:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in thinking.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), Style::default().fg(Color::Yellow)),
                ]));
            }
            lines.push(Line::from(""));
        }
        ContentItem::Image { source } => {
            lines.push(Line::from(Span::styled(
                "Image:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(format!("  Type: {}", source.media_type)));
            lines.push(Line::from(format!("  Size: {} bytes", source.data.len())));
            lines.push(Line::from(""));
        }
    }
}

fn format_usage_info(usage: &UsageInfo, lines: &mut Vec<Line>) {
    lines.push(Line::from(Span::styled(
        "Token Usage:",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    if let Some(input) = usage.input_tokens {
        lines.push(Line::from(format!("  Input tokens: {}", input)));
    }
    if let Some(cache_creation) = usage.cache_creation_input_tokens {
        lines.push(Line::from(format!(
            "  Cache creation tokens: {}",
            cache_creation
        )));
    }
    if let Some(cache_read) = usage.cache_read_input_tokens {
        lines.push(Line::from(format!("  Cache read tokens: {}", cache_read)));
    }
    if let Some(output) = usage.output_tokens {
        lines.push(Line::from(format!("  Output tokens: {}", output)));
    }

    lines.push(Line::from(vec![
        Span::raw("  Total: "),
        Span::styled(
            format!("{}", usage.total_tokens()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));
}
