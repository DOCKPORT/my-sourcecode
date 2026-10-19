use crate::scan::LanguageRow;
use crate::ui::format::{average, commas, percent};
use crate::ui::table::draw_table;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph};

/// The read-only language summary shown above the file table.
pub struct LanguageSummary {
    rows: Vec<LanguageRow>,
    /// Total bytes of all counted files, used for the share column.
    total_bytes: usize,
}

impl LanguageSummary {
    /// Build the summary from the scan rows.
    pub fn new(rows: Vec<LanguageRow>) -> Self {
        let total_bytes = rows.iter().map(|row| row.bytes).sum();
        Self { rows, total_bytes }
    }

    /// Number of languages in the summary.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Draw the summary into the given area.
    ///
    /// When the rows do not fit, the last row reports how many languages are
    /// hidden. The file table stays the only interactive screen.
    pub fn draw(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Languages ({}) ", self.rows.len()));

        if self.rows.is_empty() {
            let message =
                Paragraph::new(Text::raw("No languages found in this repository.")).block(block);
            frame.render_widget(message, area);
            return;
        }

        let header = [
            "language", "files", "lines", "code", "comments", "blanks", "bytes", "share", "avg",
        ]
        .map(str::to_string)
        .to_vec();

        // Two borders and two header lines leave the body area. Each language
        // uses two lines because one blank line sits between entries.
        let body_height = area.height.saturating_sub(4) as usize;
        let max_entries = (body_height + 1) / 2;
        let overflow = self.rows.len().saturating_sub(max_entries);
        let shown = if overflow > 0 {
            max_entries.saturating_sub(1)
        } else {
            self.rows.len()
        };

        let mut table_rows: Vec<Vec<String>> = Vec::new();
        for (index, row) in self.rows.iter().take(shown).enumerate() {
            if index > 0 {
                table_rows.push(blank_row());
            }
            table_rows.push(vec![
                row.language.clone(),
                commas(row.files),
                commas(row.lines),
                commas(row.code),
                commas(row.comments),
                commas(row.blanks),
                commas(row.bytes),
                percent(row.bytes, self.total_bytes),
                average(row.lines, row.files),
            ]);
        }

        if overflow > 0 {
            if shown > 0 {
                table_rows.push(blank_row());
            }
            let mut cells = vec![format!("+ {overflow} more languages")];
            cells.extend((0..8).map(|_| String::new()));
            table_rows.push(cells);
        }

        let widths = [
            Constraint::Min(14),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(7),
        ];

        let centered = [false; 9];

        draw_table(
            frame,
            area,
            block,
            &header,
            table_rows,
            &widths,
            &centered,
            true,
            false,
            None,
        );
    }
}

/// A blank separator row with one cell per language column.
fn blank_row() -> Vec<String> {
    (0..9).map(|_| String::new()).collect()
}
