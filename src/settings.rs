//! Claude Code settings.json parser and analyzer.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Claude project settings from settings.json or settings.local.json
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    #[serde(default)]
    pub enable_all_project_mcp_servers: bool,
    #[serde(default)]
    pub permissions: Permissions,
}

/// Permission rules for tools and operations
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Permissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
}

/// Parsed permission entry
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Bash command: Bash(command:args)
    Bash(String),
    /// WebFetch domain: WebFetch(domain:example.com)
    WebFetch(String),
    /// WebSearch (no args)
    WebSearch,
    /// Generic tool permission: ToolName(pattern)
    Tool { name: String, pattern: String },
    /// Simple permission (no pattern)
    Simple(String),
}

impl Permission {
    /// Parse a permission string like "Bash(cargo:*)" or "WebSearch"
    pub fn parse(s: &str) -> Self {
        // Check for simple permissions first
        if !s.contains('(') {
            return match s {
                "WebSearch" => Permission::WebSearch,
                _ => Permission::Simple(s.to_string()),
            };
        }

        // Parse Tool(pattern) format
        if let Some(open_paren) = s.find('(') {
            if let Some(close_paren) = s.rfind(')') {
                let tool_name = &s[..open_paren];
                let pattern = &s[open_paren + 1..close_paren];

                match tool_name {
                    "Bash" => Permission::Bash(pattern.to_string()),
                    "WebFetch" => Permission::WebFetch(pattern.to_string()),
                    _ => Permission::Tool {
                        name: tool_name.to_string(),
                        pattern: pattern.to_string(),
                    },
                }
            } else {
                Permission::Simple(s.to_string())
            }
        } else {
            Permission::Simple(s.to_string())
        }
    }

    /// Get the tool name
    pub fn tool_name(&self) -> &str {
        match self {
            Permission::Bash(_) => "Bash",
            Permission::WebFetch(_) => "WebFetch",
            Permission::WebSearch => "WebSearch",
            Permission::Tool { name, .. } => name,
            Permission::Simple(s) => s,
        }
    }

    /// Get the pattern if any
    pub fn pattern(&self) -> Option<&str> {
        match self {
            Permission::Bash(p) => Some(p),
            Permission::WebFetch(p) => Some(p),
            Permission::Tool { pattern, .. } => Some(pattern),
            Permission::WebSearch | Permission::Simple(_) => None,
        }
    }

    /// Format back to string
    pub fn to_string(&self) -> String {
        match self {
            Permission::Bash(p) => format!("Bash({})", p),
            Permission::WebFetch(p) => format!("WebFetch({})", p),
            Permission::WebSearch => "WebSearch".to_string(),
            Permission::Tool { name, pattern } => format!("{}({})", name, pattern),
            Permission::Simple(s) => s.clone(),
        }
    }
}

/// Load settings from a Claude project directory
pub fn load_project_settings<P: AsRef<Path>>(project_dir: P) -> Result<ProjectSettings> {
    let project_dir = project_dir.as_ref();
    let claude_dir = project_dir.join(".claude");

    // Try settings.local.json first, then settings.json
    let local_settings = claude_dir.join("settings.local.json");
    let global_settings = claude_dir.join("settings.json");

    let settings_path = if local_settings.exists() {
        local_settings
    } else if global_settings.exists() {
        global_settings
    } else {
        anyhow::bail!(
            "No settings file found in {}",
            claude_dir.display()
        );
    };

    let content = std::fs::read_to_string(&settings_path)
        .with_context(|| format!("Failed to read {}", settings_path.display()))?;

    let settings: ProjectSettings = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", settings_path.display()))?;

    Ok(settings)
}

/// Find all Claude project directories
pub fn find_project_dirs<P: AsRef<Path>>(base_dir: P) -> Result<Vec<PathBuf>> {
    let base_dir = base_dir.as_ref();
    let mut projects = Vec::new();

    if !base_dir.exists() {
        return Ok(projects);
    }

    for entry in std::fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let claude_dir = path.join(".claude");
            // Check if .claude directory exists and has a settings file
            if claude_dir.exists() {
                let has_settings = claude_dir.join("settings.local.json").exists()
                    || claude_dir.join("settings.json").exists();
                if has_settings {
                    projects.push(path);
                }
            }
        }
    }

    Ok(projects)
}

/// Aggregate permissions across multiple projects
#[derive(Debug, Default)]
pub struct PermissionAggregation {
    /// Permission -> list of projects that use it
    pub allow_map: HashMap<Permission, Vec<String>>,
    pub deny_map: HashMap<Permission, Vec<String>>,
    pub ask_map: HashMap<Permission, Vec<String>>,
    /// Projects with MCP enabled
    pub mcp_enabled_projects: Vec<String>,
}

impl PermissionAggregation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a project's settings to the aggregation
    pub fn add_project(&mut self, project_name: String, settings: &ProjectSettings) {
        // Track MCP settings
        if settings.enable_all_project_mcp_servers {
            self.mcp_enabled_projects.push(project_name.clone());
        }

        // Aggregate allow permissions
        for perm_str in &settings.permissions.allow {
            let perm = Permission::parse(perm_str);
            self.allow_map
                .entry(perm)
                .or_insert_with(Vec::new)
                .push(project_name.clone());
        }

        // Aggregate deny permissions
        for perm_str in &settings.permissions.deny {
            let perm = Permission::parse(perm_str);
            self.deny_map
                .entry(perm)
                .or_insert_with(Vec::new)
                .push(project_name.clone());
        }

        // Aggregate ask permissions
        for perm_str in &settings.permissions.ask {
            let perm = Permission::parse(perm_str);
            self.ask_map
                .entry(perm)
                .or_insert_with(Vec::new)
                .push(project_name.clone());
        }
    }

    /// Get top N most common permissions
    pub fn top_allowed(&self, n: usize) -> Vec<(Permission, usize)> {
        let mut perms: Vec<_> = self
            .allow_map
            .iter()
            .map(|(perm, projects)| (perm.clone(), projects.len()))
            .collect();
        perms.sort_by(|a, b| b.1.cmp(&a.1));
        perms.into_iter().take(n).collect()
    }

    /// Get top N most common denied permissions
    pub fn top_denied(&self, n: usize) -> Vec<(Permission, usize)> {
        let mut perms: Vec<_> = self
            .deny_map
            .iter()
            .map(|(perm, projects)| (perm.clone(), projects.len()))
            .collect();
        perms.sort_by(|a, b| b.1.cmp(&a.1));
        perms.into_iter().take(n).collect()
    }

    /// Get total project count
    pub fn total_projects(&self) -> usize {
        let mut projects = std::collections::HashSet::new();
        for proj_list in self.allow_map.values() {
            projects.extend(proj_list.iter().cloned());
        }
        for proj_list in self.deny_map.values() {
            projects.extend(proj_list.iter().cloned());
        }
        for proj_list in self.ask_map.values() {
            projects.extend(proj_list.iter().cloned());
        }
        projects.len()
    }
}

/// Aggregate settings from all projects in a directory
pub fn aggregate_settings<P: AsRef<Path>>(base_dir: P) -> Result<PermissionAggregation> {
    let projects = find_project_dirs(&base_dir)?;
    let mut agg = PermissionAggregation::new();

    for project_path in projects {
        let project_name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        match load_project_settings(&project_path) {
            Ok(settings) => {
                agg.add_project(project_name, &settings);
            }
            Err(e) => {
                eprintln!("Warning: Failed to load settings for {}: {}", project_name, e);
            }
        }
    }

    Ok(agg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bash_permission() {
        let perm = Permission::parse("Bash(cargo build:*)");
        assert_eq!(perm, Permission::Bash("cargo build:*".to_string()));
        assert_eq!(perm.tool_name(), "Bash");
        assert_eq!(perm.pattern(), Some("cargo build:*"));
    }

    #[test]
    fn test_parse_webfetch_permission() {
        let perm = Permission::parse("WebFetch(domain:example.com)");
        assert_eq!(perm, Permission::WebFetch("domain:example.com".to_string()));
    }

    #[test]
    fn test_parse_websearch_permission() {
        let perm = Permission::parse("WebSearch");
        assert_eq!(perm, Permission::WebSearch);
        assert_eq!(perm.pattern(), None);
    }

    #[test]
    fn test_parse_generic_tool() {
        let perm = Permission::parse("Read(*.env)");
        assert!(matches!(perm, Permission::Tool { .. }));
    }
}
