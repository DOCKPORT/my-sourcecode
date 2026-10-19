use crate::scan::LanguageRow;
use crate::ui::Action;
use crate::ui::bindings::Command;
use crate::ui::format::{average, commas, percent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

/// How many rows one page key jumps at a time.
const PAGE_JUMP: isize = 50;

/// The summary table of languages, with their share of total lines.
pub struct LanguageTable {
    /// One row per language from the scan.
    rows: Vec<LanguageRow>,
    /// Selection and scroll position over `rows`.
    state: TableState,
    /// Total lines across all languages, used for the share column.
    total_lines: usize,
}

impl LanguageTable {
    /// Build the summary from the scan rows. The first row starts selected.
    pub fn new(rows: Vec<LanguageRow>) -> Self {
        let total_lines = rows.iter().map(|row| row.lines).sum();
        let mut state = TableState::default();
        state.select(if rows.is_empty() { None } else { Some(0) });
        Self {
            rows,
            state,
            total_lines,
        }
    }

    /// Number of languages in the table.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// One-based position of the selected row, or 0 when the table is empty.
    pub fn cursor(&self) -> usize {
        self.state.selected().map_or(0, |index| index + 1)
    }

    /// Handle one command. Returns an action only when the command leaves this
    /// screen. Key-to-command mapping lives in the bindings module.
    pub fn handle(&mut self, command: Command) -> Option<Action> {
        match command {
            Command::MoveUp => {
                self.move_selection(-1);
                None
            }
            Command::MoveDown => {
                self.move_selection(1);
                None
            }
            Command::PageUp => {
                self.move_selection(-PAGE_JUMP);
                None
            }
            Command::PageDown => {
                self.move_selection(PAGE_JUMP);
                None
            }
            Command::Back => Some(Action::BackToPicker),
            Command::NextView => Some(Action::NextView),
            Command::PreviousView => Some(Action::PreviousView),
            Command::Quit => Some(Action::Quit),
            Command::Type(_)
            | Command::EraseChar
            | Command::Open
            | Command::OpenRecent
            | Command::SortColumnLeft
            | Command::SortColumnRight
            | Command::OrderToggle
            | Command::StartSearch => None,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let current = self.state.selected().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.rows.len() as isize - 1);
        self.state.select(Some(next as usize));
    }

    /// Draw the summary into the given area. The mutable state lets ratatui
    /// keep the scroll offset while the selection moves past the visible rows.
    pub fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Languages ({}) ", self.rows.len()));

        if self.rows.is_empty() {
            let message =
                Paragraph::new(Text::raw("No languages found in this repository.")).block(block);
            frame.render_widget(message, area);
            return;
        }

        let header = Row::new(vec![
            Cell::from(Text::raw("language")),
            Cell::from(Text::raw("files")),
            Cell::from(Text::raw("lines")),
            Cell::from(Text::raw("code")),
            Cell::from(Text::raw("comments")),
            Cell::from(Text::raw("blanks")),
            Cell::from(Text::raw("share")),
            Cell::from(Text::raw("avg/file")),
        ]);

        let rows: Vec<Row> = self
            .rows
            .iter()
            .map(|row| {
                Row::new(vec![
                    Cell::from(Text::raw(row.language.clone())),
                    Cell::from(Text::raw(commas(row.files))),
                    Cell::from(Text::raw(commas(row.lines))),
                    Cell::from(Text::raw(commas(row.code))),
                    Cell::from(Text::raw(commas(row.comments))),
                    Cell::from(Text::raw(commas(row.blanks))),
                    Cell::from(Text::raw(percent(row.lines, self.total_lines))),
                    Cell::from(Text::raw(average(row.lines, row.files))),
                ])
            })
            .collect();

        let widths = [
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(9),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .highlight_symbol("> ")
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            )
            .block(block);

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}
