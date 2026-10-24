use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rayon::prelude::*;

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

/// One real file and its size at scan time.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: usize,
}

/// List tracked files and untracked files that git does not ignore.
///
/// `git ls-files` applies the repository `.gitignore` rules, so the file list
/// is already limited to files the repository owns. Entries that are not real
/// files, such as deleted index entries and submodule gitlinks, are dropped.
/// The size comes from the same metadata call that checks the file type.
pub fn list_files(root: &Path) -> Result<Vec<FileEntry>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git ls-files failed: {}", message.trim()));
    }

    let paths: Vec<PathBuf> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| root.join(String::from_utf8_lossy(entry).as_ref()))
        .collect();

    let mut files: Vec<FileEntry> = paths
        .into_par_iter()
        .filter_map(|path| {
            let metadata = std::fs::metadata(&path).ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some(FileEntry {
                path,
                size: metadata.len() as usize,
            })
        })
        .collect();

    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.dedup_by(|left, right| left.path == right.path);
    Ok(files)
}

/// Check whether any repository `.gitattributes` file mentions a Linguist
/// attribute. Without such a rule, `git check-attr` cannot change the scan.
pub fn has_linguist_attributes(files: &[FileEntry]) -> bool {
    files
        .iter()
        .filter(|file| file.path.file_name() == Some(std::ffi::OsStr::new(".gitattributes")))
        .any(|file| {
            let Ok(data) = std::fs::read(&file.path) else {
                return false;
            };
            let text = String::from_utf8_lossy(&data);
            text.lines().any(|line| {
                let line = line.trim_start();
                !line.is_empty() && !line.starts_with('#') && line.contains("linguist-")
            })
        })
}

/// Linguist attributes for one file, from `.gitattributes`.
///
/// A flag is `None` when git reports `unspecified`. A flag is `Some(false)`
/// when the attribute is unset, for example `-linguist-vendored`.
#[derive(Debug, Default, Clone)]
pub struct LinguistAttributes {
    pub generated: Option<bool>,
    pub vendored: Option<bool>,
    pub documentation: Option<bool>,
    pub language: Option<String>,
}

/// Read the Linguist attributes for all files in one git call.
///
/// A failure returns an empty map, so attribute support never blocks a scan.
pub fn linguist_attributes(
    root: &Path,
    files: &[FileEntry],
) -> HashMap<PathBuf, LinguistAttributes> {
    let mut attributes = HashMap::new();
    if files.is_empty() {
        return attributes;
    }

    let mut child = match Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "check-attr",
            "-z",
            "--stdin",
            "linguist-generated",
            "linguist-vendored",
            "linguist-documentation",
            "linguist-language",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return attributes,
    };

    let paths: Vec<String> = files
        .iter()
        .filter_map(|file| {
            file.path
                .strip_prefix(root)
                .ok()
                .map(|relative| relative.to_string_lossy().into_owned())
        })
        .collect();
    let stdin = child.stdin.take();
    // Write in a thread so a large attribute response cannot block the pipe.
    let writer = std::thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            for path in paths {
                let _ = stdin.write_all(path.as_bytes());
                let _ = stdin.write_all(&[0]);
            }
        }
    });

    let Ok(output) = child.wait_with_output() else {
        let _ = writer.join();
        return attributes;
    };
    let _ = writer.join();
    if !output.status.success() {
        return attributes;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.split('\0');
    while let (Some(path), Some(attribute), Some(value)) =
        (fields.next(), fields.next(), fields.next())
    {
        if path.is_empty() {
            break;
        }
        let entry = attributes.entry(root.join(path)).or_default();
        match attribute {
            "linguist-generated" => entry.generated = attribute_flag(value),
            "linguist-vendored" => entry.vendored = attribute_flag(value),
            "linguist-documentation" => entry.documentation = attribute_flag(value),
            "linguist-language"
                if value != "unspecified" && value != "unset" && !value.is_empty() =>
            {
                entry.language = Some(value.to_string());
            }
            _ => {}
        }
    }

    attributes
}

fn attribute_flag(value: &str) -> Option<bool> {
    if value == "unspecified" {
        None
    } else {
        Some(matches!(value, "true" | "set" | "yes"))
    }
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

impl GitStats {
    /// Neutral values for a failed or unavailable history read.
    pub fn empty() -> Self {
        Self {
            branch: "none".to_string(),
            commits: 0,
            tags: 0,
            latest_tag: "none".to_string(),
            first_commit: "none".to_string(),
            last_commit: "none".to_string(),
        }
    }
}

/// Collect the repository information. Missing values fall back to safe text
/// or zero, so an empty repository still shows a table.
pub fn stats(root: &Path) -> GitStats {
    // The commands read the same repository, so run them together. Wall time
    // then follows the slowest command, not the sum of all five.
    std::thread::scope(|scope| {
        let branch = scope.spawn(move || {
            run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "none".to_string())
        });
        let commits = scope.spawn(move || {
            run_git(root, &["rev-list", "--count", "HEAD"])
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0)
        });
        let tags = scope.spawn(move || {
            let text = run_git(root, &["tag", "--sort=-creatordate"]).unwrap_or_default();
            let count = text.lines().filter(|line| !line.is_empty()).count();
            let latest = text
                .lines()
                .find(|line| !line.is_empty())
                .unwrap_or("none")
                .to_string();
            (count, latest)
        });
        let first_commit = scope.spawn(move || {
            run_git(root, &["log", "--max-parents=0", "--format=%cs", "HEAD"])
                .and_then(|value| {
                    value
                        .lines()
                        .find(|line| !line.is_empty())
                        .map(str::to_string)
                })
                .map(|value| format_date(&value))
                .unwrap_or_else(|| "none".to_string())
        });
        let last_commit = scope.spawn(move || {
            run_git(root, &["log", "-1", "--format=%cs"])
                .filter(|value| !value.is_empty())
                .map(|value| format_date(&value))
                .unwrap_or_else(|| "none".to_string())
        });

        let branch = branch.join().unwrap_or_else(|_| "none".to_string());
        let commits = commits.join().unwrap_or(0);
        let (tags, latest_tag) = tags.join().unwrap_or_else(|_| (0, "none".to_string()));
        let first_commit = first_commit.join().unwrap_or_else(|_| "none".to_string());
        let last_commit = last_commit.join().unwrap_or_else(|_| "none".to_string());

        GitStats {
            branch,
            commits,
            tags,
            latest_tag,
            first_commit,
            last_commit,
        }
    })
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
