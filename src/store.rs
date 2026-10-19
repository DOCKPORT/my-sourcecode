use std::fs;
use std::path::{Path, PathBuf};

/// Maximum number of recent paths kept on disk.
const MAX_RECENTS: usize = 5;

/// The most recent repository paths the user opened, persisted as JSON under
/// `~/.local/share/my-sourcecode/recents.json`.
///
/// Reading and writing never fail the application. A missing, unreadable, or
/// broken file is treated as an empty list, and a write error is ignored.
pub struct RecentPaths {
    /// Where the list is stored. None when the home directory is unknown, in
    /// which case the list lives in memory only.
    file: Option<PathBuf>,
    /// The paths, newest first.
    entries: Vec<String>,
}

impl RecentPaths {
    /// Load the saved paths. Any read or parse error yields an empty list.
    pub fn load() -> Self {
        let file = default_file();
        let entries = file.as_deref().and_then(read_entries).unwrap_or_default();
        Self { file, entries }
    }

    /// The saved paths, newest first.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Record `path` as the most recent entry.
    ///
    /// A blank path is ignored. An existing equal path moves to the front and
    /// the list is capped at [`MAX_RECENTS`]. The list is saved after every
    /// change.
    pub fn add(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        self.entries.retain(|entry| entry != path);
        self.entries.insert(0, path.to_string());
        self.entries.truncate(MAX_RECENTS);
        self.save();
    }

    /// Write the list to disk. Errors are ignored: persistence is a
    /// convenience and must never interrupt browsing.
    fn save(&self) {
        let Some(file) = &self.file else {
            return;
        };
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            let _ = fs::write(file, json);
        }
    }
}

/// The default storage path: `~/.local/share/my-sourcecode/recents.json`.
fn default_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut dir = PathBuf::from(home);
    dir.push(".local/share/my-sourcecode");
    dir.push("recents.json");
    Some(dir)
}

/// Read and parse the JSON list. Any failure yields None.
fn read_entries(file: &Path) -> Option<Vec<String>> {
    let data = fs::read_to_string(file).ok()?;
    serde_json::from_str(&data).ok()
}
