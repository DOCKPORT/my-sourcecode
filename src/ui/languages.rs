use crate::scan::LanguageRow;
use crate::ui::format::{average, commas};
use crate::ui::table::{TableSpec, draw_table};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Languages below this share (in tenths of a percent) join the Other row.
const OTHER_THRESHOLD_TENTHS: u16 = 5;

struct DisplayLanguage {
    row: LanguageRow,
    tenths: u16,
}

/// The read-only language summary shown above the file table.
pub struct LanguageSummary {
    display: Vec<DisplayLanguage>,
}

impl LanguageSummary {
    /// Build the summary from the scan rows.
    pub fn new(rows: Vec<LanguageRow>) -> Self {
        let total_bytes = rows.iter().map(|row| row.bytes).sum();
        let shares = compute_shares(&rows, total_bytes);

        let mut display = Vec::new();
        let mut other = LanguageRow {
            language: "Other".to_string(),
            files: 0,
            lines: 0,
            code: 0,
            comments: 0,
            blanks: 0,
            bytes: 0,
            color: None,
        };
        let mut other_tenths = 0u16;
        let mut has_other = false;

        for (row, tenths) in rows.into_iter().zip(shares) {
            if tenths < OTHER_THRESHOLD_TENTHS {
                has_other = true;
                other_tenths += tenths;
                other.files += row.files;
                other.lines += row.lines;
                other.code += row.code;
                other.comments += row.comments;
                other.blanks += row.blanks;
                other.bytes += row.bytes;
            } else {
                display.push(DisplayLanguage { row, tenths });
            }
        }

        if has_other {
            display.push(DisplayLanguage {
                row: other,
                tenths: other_tenths,
            });
        }

        Self { display }
    }

    /// Number of rows in the summary.
    pub fn len(&self) -> usize {
        self.display.len()
    }

    /// Every language as a display row, for the overlay.
    pub fn display_rows(&self) -> Vec<Vec<String>> {
        self.display
            .iter()
            .map(|language| self.row_cells(language.tenths, &language.row))
            .collect()
    }

    /// Every language color, for the overlay.
    pub fn display_colors(&self) -> Vec<Option<Color>> {
        self.display
            .iter()
            .map(|language| {
                language
                    .row
                    .color
                    .as_deref()
                    .and_then(theme::language_color)
            })
            .collect()
    }

    /// Build one formatted table row.
    fn row_cells(&self, tenths: u16, row: &LanguageRow) -> Vec<String> {
        vec![
            row.language.clone(),
            commas(row.files),
            commas(row.lines),
            commas(row.code),
            commas(row.comments),
            commas(row.blanks),
            commas(row.bytes),
            share_label(tenths),
            average(row.lines, row.files),
        ]
    }

    /// Draw the summary into the given area.
    ///
    /// When the rows do not fit, the last row reports how many languages are
    /// hidden. The file table stays the only interactive screen.
    pub fn draw(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Languages ({}) ", self.display.len()));

        if self.display.is_empty() {
            let message = Paragraph::new(Text::raw("No languages found in this repository."))
                .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND))
                .block(
                    block
                        .style(Style::default().bg(theme::BACKGROUND))
                        .border_style(Style::default().fg(theme::BORDER))
                        .title_style(Style::default().fg(theme::FOREGROUND)),
                );
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
        let overflow = self.display.len().saturating_sub(max_entries);
        let shown = if overflow > 0 {
            max_entries.saturating_sub(1)
        } else {
            self.display.len()
        };

        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut cell_colors: Vec<Option<Color>> = Vec::new();
        for (index, language) in self.display.iter().take(shown).enumerate() {
            if index > 0 {
                table_rows.push(blank_row());
                cell_colors.push(None);
            }
            table_rows.push(self.row_cells(language.tenths, &language.row));
            cell_colors.push(
                language
                    .row
                    .color
                    .as_deref()
                    .and_then(theme::language_color),
            );
        }

        if overflow > 0 {
            if shown > 0 {
                table_rows.push(blank_row());
                cell_colors.push(None);
            }
            let mut cells = vec![format!("+ {overflow} more languages")];
            cells.extend((0..8).map(|_| String::new()));
            table_rows.push(cells);
            cell_colors.push(None);
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
                color_column: Some(0),
                cell_colors,
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
