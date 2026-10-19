use crate::scan::{FileRow, LanguageRow};
use crate::ui::format::commas;
use crate::ui::table::draw_table;
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
        }
    }

    /// Draw the totals as a table with one data row.
    pub fn draw(&self, frame: &mut Frame<'_>, area: Rect) {
        let header = ["files", "languages", "lines", "code", "comments", "blanks"]
            .map(str::to_string)
            .to_vec();
        let row = vec![
            commas(self.files),
            commas(self.languages),
            commas(self.lines),
            commas(self.code),
            commas(self.comments),
            commas(self.blanks),
        ];

        let widths = [Constraint::Ratio(1, 6); 6];
        let centered = [false; 6];
        let block = Block::default().borders(Borders::ALL).title(" Totals ");

        draw_table(
            frame,
            area,
            block,
            &header,
            vec![row],
            &widths,
            &centered,
            false,
            true,
            None,
        );
    }
}
