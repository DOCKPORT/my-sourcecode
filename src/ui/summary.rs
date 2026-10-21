use crate::scan::{FileRow, LanguageRow};
use crate::ui::format::{average, commas};
use crate::ui::table::{TableSpec, draw_table};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, Borders};

/// The repository totals shown above the file table.
pub struct Summary {
    files: usize,
    languages: usize,
    lines: usize,
    code: usize,
    comments: usize,
    blanks: usize,
    bytes: usize,
}

impl Summary {
    /// Total the file rows and count the languages.
    pub fn new(files: &[FileRow], languages: &[LanguageRow]) -> Self {
        Self {
            files: files.len(),
            languages: languages.len(),
            lines: files.iter().map(|row| row.lines).sum(),
            code: files.iter().map(|row| row.code).sum(),
            comments: files.iter().map(|row| row.comments).sum(),
            blanks: files.iter().map(|row| row.blanks).sum(),
            bytes: languages.iter().map(|row| row.bytes).sum(),
        }
    }

    /// Draw the totals as a table with one data row.
    pub fn draw(&self, frame: &mut Frame<'_>, area: Rect) {
        let header = [
            "files",
            "languages",
            "lines",
            "code",
            "comments",
            "blanks",
            "bytes",
            "avg lines/file",
        ]
        .map(str::to_string)
        .to_vec();
        let row = vec![
            commas(self.files),
            commas(self.languages),
            commas(self.lines),
            commas(self.code),
            commas(self.comments),
            commas(self.blanks),
            commas(self.bytes),
            average(self.lines, self.files),
        ];

        let widths = [Constraint::Ratio(1, 8); 8];
        let centered = [false; 8];
        let block = Block::default().borders(Borders::ALL).title(" Totals ");

        draw_table(
            frame,
            area,
            TableSpec {
                block,
                header,
                rows: vec![row],
                widths: widths.to_vec(),
                left_aligned: centered.to_vec(),
                row_rules: false,
                body_dividers: true,
                color_column: None,
                cell_colors: Vec::new(),
            },
            None,
        );
    }
}
