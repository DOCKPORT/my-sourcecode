use crate::scan::LanguageRow;
use crate::ui::format::{average, commas};
use crate::ui::table::{TableSpec, draw_table};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph};

/// The read-only language summary shown above the file table.
pub struct LanguageSummary {
    rows: Vec<LanguageRow>,
    /// Share per language in tenths of a percent. The values sum to 1000.
    shares: Vec<u16>,
}

impl LanguageSummary {
    /// Build the summary from the scan rows.
    pub fn new(rows: Vec<LanguageRow>) -> Self {
        let total_bytes = rows.iter().map(|row| row.bytes).sum();
        let shares = compute_shares(&rows, total_bytes);
        Self { rows, shares }
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
            "language",
            "files",
            "lines",
            "code",
            "comments",
            "blanks",
            "bytes",
            "share",
            "avg lines/file",
        ]
        .map(str::to_string)
        .to_vec();

        // Two borders and two header lines leave the body area. Each language
        // uses two lines because one blank line sits between entries.
        let body_height = area.height.saturating_sub(4) as usize;
        let max_entries = body_height.div_ceil(2);
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
                share_label(self.shares.get(index).copied().unwrap_or(0)),
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
            Constraint::Length(15),
        ];

        let centered = [false; 9];

        draw_table(
            frame,
            area,
            TableSpec {
                block,
                header,
                rows: table_rows,
                widths: widths.to_vec(),
                left_aligned: centered.to_vec(),
                row_rules: true,
                body_dividers: false,
            },
            None,
        );
    }
}

/// A blank separator row with one cell per language column.
fn blank_row() -> Vec<String> {
    (0..9).map(|_| String::new()).collect()
}

/// Compute each share in tenths of a percent.
///
/// Each share rounds to the nearest tenth. The largest language then absorbs the
/// difference from 1000 tenths, so the visible values always sum to 100.0%.
fn compute_shares(rows: &[LanguageRow], total_bytes: usize) -> Vec<u16> {
    if rows.is_empty() {
        return Vec::new();
    }
    if total_bytes == 0 {
        return vec![0; rows.len()];
    }

    let total = total_bytes as u128;
    let half = total / 2;
    let mut tenths: Vec<u16> = Vec::with_capacity(rows.len());

    for row in rows {
        let scaled = row.bytes as u128 * 1000;
        tenths.push(((scaled + half) / total) as u16);
    }

    let assigned: u32 = tenths.iter().map(|value| *value as u32).sum();
    let drift = 1000i32 - assigned as i32;
    if drift != 0 && !rows.is_empty() {
        let mut largest = 0usize;
        for (index, row) in rows.iter().enumerate() {
            if row.bytes > rows[largest].bytes
                || (row.bytes == rows[largest].bytes && row.language < rows[largest].language)
            {
                largest = index;
            }
        }
        tenths[largest] = (tenths[largest] as i32 + drift).clamp(0, 1000) as u16;
    }

    tenths
}

/// Format share tenths as a percentage with one decimal place.
fn share_label(tenths: u16) -> String {
    format!("{}.{}%", tenths / 10, tenths % 10)
}
