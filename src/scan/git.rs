use std::path::{Path, PathBuf};
use std::process::Command;

/// Stop if `root` is not inside a git working tree.
pub fn verify_repository(root: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;

    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "true" {
        return Err(format!("not a git repository: {}", root.display()));
    }

    Ok(())
}

/// List tracked files and untracked files that git does not ignore.
///
/// `git ls-files` applies the repository `.gitignore` rules, so the file list
/// is already limited to files the repository owns. Entries that are not real
/// files, such as deleted index entries and submodule gitlinks, are dropped.
pub fn list_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git ls-files failed: {}", message.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut files: Vec<PathBuf> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(|line| root.join(line))
        .filter(|path| path.is_file())
        .collect();

    files.sort();
    files.dedup();
    Ok(files)
}

/// Read-only repository information for the Git table.
#[derive(Debug, Clone)]
pub struct GitStats {
    pub branch: String,
    pub commits: usize,
    pub tags: usize,
    pub latest_tag: String,
    pub first_commit: String,
    pub last_commit: String,
}

/// Collect the repository information. Missing values fall back to safe text
/// or zero, so an empty repository still shows a table.
pub fn stats(root: &Path) -> GitStats {
    let branch = run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_string());

    let commits = run_git(root, &["rev-list", "--count", "HEAD"])
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    let tag_text = run_git(root, &["tag", "--sort=-creatordate"]).unwrap_or_default();
    let tags = tag_text.lines().filter(|line| !line.is_empty()).count();
    let latest_tag = tag_text
        .lines()
        .find(|line| !line.is_empty())
        .unwrap_or("none")
        .to_string();

    let first_commit = run_git(root, &["log", "--max-parents=0", "--format=%cs", "HEAD"])
        .and_then(|value| {
            value
                .lines()
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .map(|value| format_date(&value))
        .unwrap_or_else(|| "none".to_string());

    let last_commit = run_git(root, &["log", "-1", "--format=%cs"])
        .filter(|value| !value.is_empty())
        .map(|value| format_date(&value))
        .unwrap_or_else(|| "none".to_string());

    GitStats {
        branch,
        commits,
        tags,
        latest_tag,
        first_commit,
        last_commit,
    }
}

/// Convert a git date from `YYYY-MM-DD` to `MM-DD-YYYY`.
///
/// A value with another shape returns unchanged, so `none` and unusual dates
/// stay readable.
fn format_date(value: &str) -> String {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return value.to_string();
    }

    format!("{}-{}-{}", parts[1], parts[2], parts[0])
}

/// Run one git command in `root`. A failed command yields None.
fn run_git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
