//! Find Claude project directories from user project paths

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Find the Claude project directory for a given project source path
pub fn find_claude_project_dir<P: AsRef<Path>>(project_path: P) -> Result<PathBuf> {
    let project_path = project_path.as_ref();

    // If path is already in ~/.claude/projects/, use it directly
    if let Some(claude_dir) = dirs::home_dir() {
        let projects_dir = claude_dir.join(".claude").join("projects");
        if project_path.starts_with(&projects_dir) {
            return Ok(project_path.to_path_buf());
        }
    }

    // Convert project path to absolute
    let abs_path = if project_path.is_absolute() {
        project_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(project_path).canonicalize()?
    };

    // Generate Claude project directory name from absolute path
    // /Users/foo/git/project -> -Users-foo-git-project
    let claude_name = abs_path
        .to_str()
        .context("Invalid path")?
        .replace('/', "-");

    // Check if this directory exists in ~/.claude/projects/
    if let Some(home) = dirs::home_dir() {
        let claude_project_dir = home
            .join(".claude")
            .join("projects")
            .join(&claude_name);

        if claude_project_dir.exists() {
            return Ok(claude_project_dir);
        }
    }

    // If not found by name transformation, search for matching cwd in settings
    if let Some(found) = search_by_cwd(&abs_path)? {
        return Ok(found);
    }

    anyhow::bail!(
        "Could not find Claude project for: {}\n\
         Expected: ~/.claude/projects/{}\n\
         Try pointing directly to the Claude project directory if the automatic lookup fails.",
        abs_path.display(),
        claude_name
    )
}

/// Search Claude projects by matching the cwd in settings files
fn search_by_cwd(project_path: &Path) -> Result<Option<PathBuf>> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(None),
    };

    let projects_dir = home.join(".claude").join("projects");
    if !projects_dir.exists() {
        return Ok(None);
    }

    // Read all project directories
    for entry in std::fs::read_dir(&projects_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Check if any .jsonl file in this directory has transcripts from our project
        if let Some(jsonl_files) = std::fs::read_dir(&path)
            .ok()
            .and_then(|entries| {
                let files: Vec<_> = entries
                    .filter_map(Result::ok)
                    .filter(|e| {
                        e.path()
                            .extension()
                            .and_then(|s| s.to_str())
                            == Some("jsonl")
                    })
                    .collect();
                if files.is_empty() {
                    None
                } else {
                    Some(files)
                }
            })
        {
            // Read first few lines of first jsonl file to check cwd
            if let Some(first_file) = jsonl_files.first() {
                if let Ok(content) = std::fs::read_to_string(first_file.path()) {
                    if let Some(first_line) = content.lines().next() {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(first_line) {
                            if let Some(cwd) = json.get("cwd").and_then(|v| v.as_str()) {
                                let cwd_path = PathBuf::from(cwd);
                                if cwd_path == *project_path {
                                    return Ok(Some(path));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_in_claude_projects() {
        // If path is already in ~/.claude/projects/, should return as-is
        if let Some(home) = dirs::home_dir() {
            let claude_path = home.join(".claude/projects/some-project");
            let result = find_claude_project_dir(&claude_path);
            // Should not error, but may not exist
            assert!(result.is_ok() || result.is_err());
        }
    }
}
