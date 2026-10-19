use std::path::PathBuf;

use tokei::{Config, Languages};

/// Count all given files. The file list already follows the git ignore rules,
/// so tokei must not apply ignore rules a second time.
pub fn count_files(files: &[PathBuf]) -> Languages {
    let config = Config {
        no_ignore: Some(true),
        hidden: Some(true),
        ..Config::default()
    };

    let mut languages = Languages::new();
    languages.get_statistics(files, &[], &config);
    languages
}
