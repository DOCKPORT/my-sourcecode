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
