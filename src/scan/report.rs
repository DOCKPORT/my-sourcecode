use std::cmp::Reverse;
use std::path::{Path, PathBuf};

use crate::scan::count::count_files;
use crate::scan::filter::LanguageFilter;
use crate::scan::git;

/// One counted file, ready for display.
pub struct FileRow {
    pub language: String,
    pub path: String,
    pub lines: usize,
    pub code: usize,
    pub comments: usize,
    pub blanks: usize,
}

/// Totals for one language, ready for display.
pub struct LanguageRow {
    pub language: String,
    pub files: usize,
    pub lines: usize,
    pub code: usize,
    pub comments: usize,
    pub blanks: usize,
    pub bytes: usize,
}

/// The complete scan result for one repository.
pub struct Report {
    /// The repository root that was scanned.
    pub root: PathBuf,
    /// One row per counted file, sorted by code lines.
    pub files: Vec<FileRow>,
    /// One row per language, sorted by total lines.
    pub languages: Vec<LanguageRow>,
    /// Read-only repository information for the Git table.
    pub git: git::GitStats,
}

/// Scan a local git repository: verify it, list its files, filter them through
/// the embedded language rules, and count the survivors with tokei.
///
/// An empty repository is not an error. The returned report then holds no
/// rows, and the interface shows the empty state.
pub fn scan(root: &Path) -> Result<Report, String> {
    git::verify_repository(root)?;
    let git_stats = git::stats(root);

    let filter = LanguageFilter::from_embedded()?;
    let files: Vec<PathBuf> = git::list_files(root)?
        .into_iter()
        .filter(|path| filter.allows(path))
        .collect();

    if files.is_empty() {
        return Ok(Report {
            root: root.to_path_buf(),
            files: Vec::new(),
            languages: Vec::new(),
            git: git_stats,
        });
    }

    let languages = count_files(&files);
    let mut file_rows: Vec<FileRow> = Vec::new();
    let mut language_rows: Vec<LanguageRow> = Vec::new();

    for (language_type, language) in languages.iter() {
        let language_name = language_type.to_string();

        if !filter.allows_language(&language_name) {
            continue;
        }

        let mut language_bytes = 0usize;

        for report in &language.reports {
            // GitHub Linguist measures the language share by file bytes. The
            // file is already in the git file list, so a metadata call is
            // enough. An unreadable file adds no bytes.
            let bytes = std::fs::metadata(&report.name)
                .map(|metadata| metadata.len() as usize)
                .unwrap_or(0);
            language_bytes += bytes;

            file_rows.push(FileRow {
                language: language_name.clone(),
                path: display_path(root, &report.name),
                lines: report.stats.lines(),
                code: report.stats.code,
                comments: report.stats.comments,
                blanks: report.stats.blanks,
            });
        }

        language_rows.push(LanguageRow {
            language: language_name,
            files: language.reports.len(),
            lines: language.lines(),
            code: language.code,
            comments: language.comments,
            blanks: language.blanks,
            bytes: language_bytes,
        });
    }

    file_rows.sort_by_key(|row| (Reverse(row.code), Reverse(row.lines), row.path.clone()));
    language_rows.sort_by_key(|row| (Reverse(row.bytes), Reverse(row.lines), row.language.clone()));

    Ok(Report {
        root: root.to_path_buf(),
        files: file_rows,
        languages: language_rows,
        git: git_stats,
    })
}

/// Show a file path relative to the repository root.
fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
