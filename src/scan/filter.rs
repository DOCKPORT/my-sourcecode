use std::collections::HashSet;

use serde::Deserialize;

/// The GitHub Linguist language rules, embedded into the binary at build time.
const LANGUAGES_YAML: &str = include_str!("../../languages.yaml");

#[derive(Deserialize)]
struct LanguageEntry {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    group: Option<String>,
}

/// Keeps files whose language type is programming or markup.
pub struct LanguageFilter {
    denied_languages: HashSet<String>,
    colors: std::collections::HashMap<String, String>,
    groups: std::collections::HashMap<String, String>,
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

        let mut denied_languages = HashSet::new();
        let mut colors = std::collections::HashMap::new();
        let mut groups = std::collections::HashMap::new();

        for (name, entry) in &entries {
            if let Some(color) = entry.color.as_deref() {
                let color = color.trim();
                if !color.is_empty() {
                    colors.insert(name.to_ascii_lowercase(), color.to_string());
                }
            }

            if let Some(group) = entry.group.as_deref() {
                let group = group.trim();
                if !group.is_empty() {
                    groups.insert(name.to_ascii_lowercase(), group.to_string());
                }
            }

            let kind = entry.kind.trim().to_ascii_lowercase();

            if kind != "programming" && kind != "markup" {
                denied_languages.insert(name.to_ascii_lowercase());
            }
        }

        Ok(Self {
            denied_languages,
            colors,
            groups,
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

    /// Resolve a Linguist group chain to its top-level language name.
    pub fn resolve_group(&self, language: &str) -> String {
        let mut current = language.trim().to_string();
        for _ in 0..8 {
            let Some(parent) = self.groups.get(&current.to_ascii_lowercase()).cloned() else {
                break;
            };
            if parent.eq_ignore_ascii_case(&current) {
                break;
            }
            current = parent;
        }
        current
    }

    /// The Linguist color for a language name, if one is defined.
    pub fn color_for(&self, language: &str) -> Option<String> {
        self.colors
            .get(&language.trim().to_ascii_lowercase())
            .cloned()
    }
}
