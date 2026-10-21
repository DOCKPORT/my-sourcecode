use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

/// The GitHub Linguist language rules, embedded into the binary at build time.
const LANGUAGES_YAML: &str = include_str!("../../languages.yaml");

#[derive(Deserialize)]
struct LanguageEntry {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    color: Option<String>,
}

/// Keeps files whose language type is programming or markup.
pub struct LanguageFilter {
    extensions: HashSet<String>,
    filenames: HashSet<String>,
    denied_languages: HashSet<String>,
    colors: std::collections::HashMap<String, String>,
}

impl LanguageFilter {
    /// Load the embedded policy file.
    pub fn from_embedded() -> Result<Self, String> {
        Self::from_yaml(LANGUAGES_YAML)
    }

    fn from_yaml(source: &str) -> Result<Self, String> {
        let entries: std::collections::HashMap<String, LanguageEntry> =
            serde_yaml::from_str(source)
                .map_err(|error| format!("cannot parse languages.yaml: {error}"))?;

        let mut extensions = HashSet::new();
        let mut filenames = HashSet::new();
        let mut denied_languages = HashSet::new();
        let mut colors = std::collections::HashMap::new();

        for (name, entry) in &entries {
            if let Some(color) = entry.color.as_deref() {
                let color = color.trim();
                if !color.is_empty() {
                    colors.insert(name.to_ascii_lowercase(), color.to_string());
                }
            }

            let kind = entry.kind.trim().to_ascii_lowercase();

            if kind != "programming" && kind != "markup" {
                denied_languages.insert(name.to_ascii_lowercase());
                continue;
            }

            for extension in &entry.extensions {
                extensions.insert(normalize_extension(extension));
            }

            for filename in &entry.filenames {
                filenames.insert(filename.to_ascii_lowercase());
            }
        }

        if extensions.is_empty() && filenames.is_empty() {
            return Err("languages.yaml has no programming or markup languages".to_string());
        }

        Ok(Self {
            extensions,
            filenames,
            denied_languages,
            colors,
        })
    }

    /// Check whether tokei detected an allowed language type.
    ///
    /// Some extensions belong to more than one Linguist language. For example,
    /// `.md` belongs to Markdown (`prose`) and GCC Machine Description
    /// (`programming`). The extension allowlist cannot separate them, so the
    /// detected language name is checked here as well.
    pub fn allows_language(&self, language: &str) -> bool {
        !self
            .denied_languages
            .contains(&language.trim().to_ascii_lowercase())
    }

    /// The Linguist color for a language name, if one is defined.
    pub fn color_for(&self, language: &str) -> Option<String> {
        self.colors
            .get(&language.trim().to_ascii_lowercase())
            .cloned()
    }

    /// Check whether a path is allowed by the policy.
    pub fn allows(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };

        let lower_name = name.to_ascii_lowercase();

        if self.filenames.contains(&lower_name) {
            return true;
        }

        lower_name
            .match_indices('.')
            .any(|(index, _)| self.extensions.contains(&lower_name[index..]))
    }
}

fn normalize_extension(extension: &str) -> String {
    let lower = extension.to_ascii_lowercase();

    if lower.starts_with('.') {
        lower
    } else {
        format!(".{lower}")
    }
}
