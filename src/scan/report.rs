use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

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
    pub color: Option<String>,
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
    pub color: Option<String>,
}

/// The complete scan result for one repository.
pub struct Report {
    /// The repository root that was scanned.
    pub root: PathBuf,
    /// One row per counted file, in the file table default order.
    pub files: Vec<FileRow>,
    /// One row per language, sorted by total lines.
    pub languages: Vec<LanguageRow>,
}

/// One stage of a scan, for progress display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Repository,
    Files,
    Detecting(usize),
    Counting(usize),
    Finishing,
}

/// Scan a local git repository: verify it, list its files, filter them through
/// the embedded language rules, and count the survivors with tokei.
///
/// An empty repository is not an error. The returned report then holds no
/// rows, and the interface shows the empty state. Each stage goes to
/// `progress` for the loading screen.
pub fn scan_with_progress<F>(root: &Path, progress: F) -> Result<Report, String>
where
    F: Fn(ScanPhase) + Sync,
{
    progress(ScanPhase::Repository);
    git::verify_repository(root)?;

    let filter = LanguageFilter::from_embedded()?;
    progress(ScanPhase::Files);
    let all_files = git::list_files(root)?;
    let attributes = if git::has_linguist_attributes(&all_files) {
        git::linguist_attributes(root, &all_files)
    } else {
        HashMap::new()
    };

    progress(ScanPhase::Detecting(all_files.len()));
    let detected: BTreeMap<PathBuf, (String, usize)> = all_files
        .par_iter()
        .filter_map(|file| {
            detect_file(&file.path, filter, attributes.get(&file.path))
                .map(|language| (file.path.clone(), (language, file.size)))
        })
        .collect();

    let files: Vec<PathBuf> = detected.keys().cloned().collect();

    if files.is_empty() {
        return Ok(Report {
            root: root.to_path_buf(),
            files: Vec::new(),
            languages: Vec::new(),
        });
    }

    progress(ScanPhase::Counting(files.len()));
    let languages = count_files(&files);
    let mut file_rows: Vec<FileRow> = Vec::new();
    let mut merged: BTreeMap<String, LanguageRow> = BTreeMap::new();
    let mut counted: BTreeSet<PathBuf> = BTreeSet::new();

    for (language_type, language) in languages.iter() {
        let tokei_name = language_type.to_string();

        for report in &language.reports {
            counted.insert(report.name.clone());

            let language_name = filter.resolve_group(
                &detected
                    .get(&report.name)
                    .map(|(name, _)| canonical_language(name))
                    .unwrap_or_else(|| canonical_language(&tokei_name)),
            );

            if !filter.allows_language(&language_name) {
                continue;
            }

            let color = filter
                .color_for(&language_name)
                .or_else(|| filter.color_for(&tokei_name));

            // GitHub Linguist measures the language share by file bytes. The
            // size comes from the one file-inventory pass.
            let bytes = detected.get(&report.name).map_or(0, |(_, size)| *size);

            let lines = report.stats.lines();
            let code = report.stats.code;
            let comments = report.stats.comments;
            let blanks = report.stats.blanks;

            file_rows.push(FileRow {
                language: language_name.clone(),
                path: display_path(root, &report.name),
                lines,
                code,
                comments,
                blanks,
                color: color.clone(),
            });

            merge_row(
                &mut merged,
                &language_name,
                color,
                1,
                lines,
                code,
                comments,
                blanks,
                bytes,
            );
        }
    }

    // Linguist can identify files that tokei cannot count, such as configure.ac
    // or Cap'n Proto. Count those files with a simple line counter so their
    // bytes still contribute to the language shares.
    for (path, (language, size)) in &detected {
        if counted.contains(path) {
            continue;
        }

        let language_name = filter.resolve_group(&canonical_language(language));
        if !filter.allows_language(&language_name) {
            continue;
        }
        let Some((lines, code, comments, blanks)) = fallback_stats(path, *size) else {
            continue;
        };
        let color = filter.color_for(&language_name);

        file_rows.push(FileRow {
            language: language_name.clone(),
            path: display_path(root, path),
            lines,
            code,
            comments,
            blanks,
            color: color.clone(),
        });
        merge_row(
            &mut merged,
            &language_name,
            color,
            1,
            lines,
            code,
            comments,
            blanks,
            *size,
        );
    }

    progress(ScanPhase::Finishing);
    let mut language_rows: Vec<LanguageRow> = merged.into_values().collect();
    file_rows.sort_by(|left, right| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| right.code.cmp(&left.code))
            .then_with(|| left.path.cmp(&right.path))
    });
    language_rows.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| right.lines.cmp(&left.lines))
            .then_with(|| left.language.cmp(&right.language))
    });

    Ok(Report {
        root: root.to_path_buf(),
        files: file_rows,
        languages: language_rows,
    })
}

/// Add one file to the language totals.
#[allow(clippy::too_many_arguments)]
fn merge_row(
    merged: &mut BTreeMap<String, LanguageRow>,
    language: &str,
    color: Option<String>,
    files: usize,
    lines: usize,
    code: usize,
    comments: usize,
    blanks: usize,
    bytes: usize,
) {
    match merged.get_mut(language) {
        Some(row) => {
            row.files += files;
            row.lines += lines;
            row.code += code;
            row.comments += comments;
            row.blanks += blanks;
            row.bytes += bytes;
            if row.color.is_none() {
                row.color = color;
            }
        }
        None => {
            merged.insert(
                language.to_string(),
                LanguageRow {
                    language: language.to_string(),
                    files,
                    lines,
                    code,
                    comments,
                    blanks,
                    bytes,
                    color,
                },
            );
        }
    }
}

/// Count a file that tokei does not support. Comments use `#` or `//`.
fn fallback_stats(path: &Path, bytes: usize) -> Option<(usize, usize, usize, usize)> {
    if bytes > 4 * 1024 * 1024 {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    if data.contains(&0) {
        return None;
    }
    let text = String::from_utf8(data).ok()?;

    let mut lines = 0usize;
    let mut code = 0usize;
    let mut comments = 0usize;
    let mut blanks = 0usize;
    for line in text.lines() {
        lines += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blanks += 1;
        } else if trimmed.starts_with('#') || trimmed.starts_with("//") {
            comments += 1;
        } else {
            code += 1;
        }
    }

    Some((lines, code, comments, blanks))
}

/// Detect the Linguist language for one file. Vendored and generated files are
/// skipped, and `.gitattributes` overrides always win.
fn detect_file(
    path: &Path,
    filter: &LanguageFilter,
    attributes: Option<&git::LinguistAttributes>,
) -> Option<String> {
    let vendored_override = attributes.and_then(|attributes| attributes.vendored);
    if vendored_override != Some(false) && linguist::is_vendored(path).unwrap_or(false) {
        return None;
    }
    if let Some(attributes) = attributes
        && (attributes.generated == Some(true)
            || attributes.vendored == Some(true)
            || attributes.documentation == Some(true))
    {
        return None;
    }

    let override_name = attributes.and_then(|attributes| attributes.language.as_deref());
    if let Some(name) = override_name {
        return filter.allows_language(name).then(|| name.to_string());
    }

    // Uppercase `.S` is preprocessed assembly. The Linguist crate matches the
    // lowercase extension only, so detect it with a temporary `.s` path.
    let detection_path = if is_uppercase_assembly(path) {
        path.with_extension("s")
    } else {
        path.to_path_buf()
    };

    // Most files resolve without content. Read only when the answer stays
    // ambiguous, when the file is unknown, or for the C header rule.
    let initial = linguist::detect_language(&detection_path, None).ok()?;
    if !is_c_header(path) {
        if initial.len() == 1 {
            let name = initial[0].name;
            return filter.allows_language(name).then(|| name.to_string());
        }
        if initial.len() > 1
            && !initial
                .iter()
                .any(|language| filter.allows_language(language.name))
        {
            return None;
        }
    }

    let contents = read_prefix(path);
    let candidates = match linguist::detect_language(&detection_path, contents.as_deref()) {
        Ok(candidates) => candidates,
        Err(_) => initial,
    };
    let mut language = candidates
        .iter()
        .find(|language| filter.allows_language(language.name))?
        .name
        .to_string();

    if is_c_header(path) && matches!(language.as_str(), "C" | "C++") {
        language = if contents.as_deref().map(looks_like_cpp).unwrap_or(false) {
            "C++".to_string()
        } else {
            "C".to_string()
        };
    }

    Some(language)
}

fn is_uppercase_assembly(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "S")
}

fn is_c_header(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("h"))
}

fn looks_like_cpp(source: &str) -> bool {
    let cleaned = strip_comments_and_strings(source);
    const MARKERS: &[&str] = &[
        "namespace ",
        "template <",
        "template<",
        "class ",
        "std::",
        "public:",
        "private:",
        "protected:",
        "constexpr",
        "noexcept",
        "using namespace",
        "friend ",
        "operator ",
    ];
    MARKERS
        .iter()
        .any(|marker| contains_token(&cleaned, marker))
}

/// Find a marker only at a token start. For example, `namespace` must not match
/// the end of `has_provision_namespace`.
fn contains_token(source: &str, marker: &str) -> bool {
    source.match_indices(marker).any(|(index, _)| {
        source[..index]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_alphanumeric() && character != '_')
    })
}

fn strip_comments_and_strings(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        LineComment,
        BlockComment,
        String(char),
        Character(char),
    }

    let mut output = String::with_capacity(source.len());
    let mut state = State::Normal;
    let mut chars = source.chars().peekable();

    while let Some(character) = chars.next() {
        match state {
            State::Normal => {
                if character == '/' && chars.peek() == Some(&'/') {
                    chars.next();
                    output.push(' ');
                    state = State::LineComment;
                } else if character == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    output.push(' ');
                    state = State::BlockComment;
                } else if character == '"' {
                    output.push(' ');
                    state = State::String('"');
                } else if character == '\'' {
                    output.push(' ');
                    state = State::Character('\'');
                } else {
                    output.push(character);
                }
            }
            State::LineComment => {
                if character == '\n' {
                    output.push('\n');
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                if character == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    output.push(' ');
                    state = State::Normal;
                }
            }
            State::String(quote) | State::Character(quote) => {
                if character == '\\' {
                    chars.next();
                } else if character == quote {
                    output.push(' ');
                    state = State::Normal;
                }
            }
        }
    }

    output
}

/// Read the start of a file as text for Linguist heuristics.
fn read_prefix(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buffer = vec![0u8; 64 * 1024];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    if buffer.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// Map tokei-only language names to GitHub Linguist names.
///
/// Linguist has one C and one C++ entry. Tokei splits header files into their
/// own languages. Linguist also has no separate JSX, Bash, Zsh, or Fish entries.
fn canonical_language(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "c header" => "C".to_string(),
        "c++ header" => "C++".to_string(),
        "jsx" => "JavaScript".to_string(),
        "bash" | "zsh" | "fish" => "Shell".to_string(),
        "gnu style assembly" => "Assembly".to_string(),
        _ => name.to_string(),
    }
}

/// Show a file path relative to the repository root.
fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
