use crate::scan::FileRow;
use crate::ui::Action;
use crate::ui::bindings::Command;
use crate::ui::format::commas;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use std::cmp::Ordering;

/// How many rows one page key jumps at a time.
const PAGE_JUMP: isize = 50;

/// The column used to sort the file table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    Path,
    Language,
    Lines,
    Code,
    Comments,
    Blanks,
}

impl SortColumn {
    fn previous(self) -> Self {
        match self {
            SortColumn::Path => SortColumn::Blanks,
            SortColumn::Language => SortColumn::Path,
            SortColumn::Lines => SortColumn::Language,
            SortColumn::Code => SortColumn::Lines,
            SortColumn::Comments => SortColumn::Code,
            SortColumn::Blanks => SortColumn::Comments,
        }
    }

    fn next(self) -> Self {
        match self {
            SortColumn::Path => SortColumn::Language,
            SortColumn::Language => SortColumn::Lines,
            SortColumn::Lines => SortColumn::Code,
            SortColumn::Code => SortColumn::Comments,
            SortColumn::Comments => SortColumn::Blanks,
            SortColumn::Blanks => SortColumn::Path,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortColumn::Path => "file",
            SortColumn::Language => "language",
            SortColumn::Lines => "lines",
            SortColumn::Code => "code",
            SortColumn::Comments => "comments",
            SortColumn::Blanks => "blanks",
        }
    }
}

/// The sortable, searchable table of counted files.
pub struct FileTable {
    /// All file rows from the scan.
    rows: Vec<FileRow>,
    /// Indices into `rows` that pass the search filter, in sort order.
    visible: Vec<usize>,
    /// Selection and scroll position over `visible`.
    state: TableState,
    /// The active sort column.
    sort_col: SortColumn,
    /// True for ascending order, false for descending.
    ascending: bool,
    /// The current search term. Empty means no filter.
    search: String,
    /// True while the user edits the search term in the footer.
    searching: bool,
}

impl FileTable {
    /// Build the table from the scan rows. The first row starts selected.
    pub fn new(rows: Vec<FileRow>) -> Self {
        let visible = (0..rows.len()).collect();
        let mut table = Self {
            rows,
            visible,
            state: TableState::default(),
            sort_col: SortColumn::Code,
            ascending: false,
            search: String::new(),
            searching: false,
        };
        table.apply_sort();
        table.state.select(if table.visible.is_empty() {
            None
        } else {
            Some(0)
        });
        table
    }

    /// True while the user edits the search term.
    pub fn searching(&self) -> bool {
        self.searching
    }

    /// The current search term.
    pub fn search_term(&self) -> &str {
        &self.search
    }

    /// Total number of files in the report.
    pub fn total(&self) -> usize {
        self.rows.len()
    }

    /// Number of files shown after filtering.
    pub fn shown(&self) -> usize {
        self.visible.len()
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
            Command::SortColumnLeft => {
                self.sort_col = self.sort_col.previous();
                self.apply_sort();
                None
            }
            Command::SortColumnRight => {
                self.sort_col = self.sort_col.next();
                self.apply_sort();
                None
            }
            Command::OrderToggle => {
                self.ascending = !self.ascending;
                self.apply_sort();
                None
            }
            Command::StartSearch => {
                self.searching = true;
                None
            }
            Command::Type(ch) => {
                if self.searching {
                    self.search.push(ch);
                    self.apply_filter();
                }
                None
            }
            Command::EraseChar => {
                if self.searching {
                    self.search.pop();
                    self.apply_filter();
                }
                None
            }
            Command::Open => {
                self.searching = false;
                None
            }
            Command::Back => {
                if self.searching || !self.search.is_empty() {
                    self.searching = false;
                    self.search.clear();
                    self.apply_filter();
                    None
                } else {
                    Some(Action::BackToPicker)
                }
            }
            Command::NextView => Some(Action::NextView),
            Command::PreviousView => Some(Action::PreviousView),
            Command::Quit => Some(Action::Quit),
            Command::OpenRecent => None,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.state.selected().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.visible.len() as isize - 1);
        self.state.select(Some(next as usize));
    }
}

impl FileTable {
    /// Sort the visible rows with the active column and direction.
    fn apply_sort(&mut self) {
        let rows = &self.rows;
        let column = self.sort_col;
        let ascending = self.ascending;
        self.visible.sort_by(|&left, &right| {
            let ordering = compare_rows(&rows[left], &rows[right], column);
            if ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });
        self.select_first();
    }

    /// Rebuild the visible rows from the search term, then sort them.
    fn apply_filter(&mut self) {
        let term = self.search.to_ascii_lowercase();
        self.visible = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                term.is_empty()
                    || row.path.to_ascii_lowercase().contains(&term)
                    || row.language.to_ascii_lowercase().contains(&term)
            })
            .map(|(index, _)| index)
            .collect();
        self.apply_sort();
    }

    fn select_first(&mut self) {
        self.state.select(if self.visible.is_empty() {
            None
        } else {
            Some(0)
        });
        *self.state.offset_mut() = 0;
    }

    /// Draw the table into the given area. The mutable state lets ratatui keep
    /// the scroll offset while the selection moves past the visible rows.
    pub fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(format!(
            " Files ({} shown of {}) ",
            self.visible.len(),
            self.rows.len()
        ));

        if self.rows.is_empty() {
            let message = Paragraph::new(Text::raw(
                "No programming or markup files found in this repository.",
            ))
            .block(block);
            frame.render_widget(message, area);
            return;
        }

        let header = Row::new(vec![
            header_cell(SortColumn::Path, self.sort_col, self.ascending),
            header_cell(SortColumn::Language, self.sort_col, self.ascending),
            header_cell(SortColumn::Lines, self.sort_col, self.ascending),
            header_cell(SortColumn::Code, self.sort_col, self.ascending),
            header_cell(SortColumn::Comments, self.sort_col, self.ascending),
            header_cell(SortColumn::Blanks, self.sort_col, self.ascending),
        ]);

        let rows: Vec<Row> = self
            .visible
            .iter()
            .map(|&index| {
                let row = &self.rows[index];
                Row::new(vec![
                    Cell::from(Text::raw(row.path.clone())),
                    Cell::from(Text::raw(row.language.clone())),
                    Cell::from(Text::raw(commas(row.lines))),
                    Cell::from(Text::raw(commas(row.code))),
                    Cell::from(Text::raw(commas(row.comments))),
                    Cell::from(Text::raw(commas(row.blanks))),
                ])
            })
            .collect();

        let widths = [
            Constraint::Min(20),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
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

fn header_cell(column: SortColumn, active: SortColumn, ascending: bool) -> Cell<'static> {
    let label = if column == active {
        let marker = if ascending { " ^" } else { " v" };
        format!("{}{marker}", column.label())
    } else {
        column.label().to_string()
    };
    Cell::from(Text::raw(label))
}

fn compare_rows(left: &FileRow, right: &FileRow, column: SortColumn) -> Ordering {
    match column {
        SortColumn::Path => left.path.cmp(&right.path),
        SortColumn::Language => left.language.cmp(&right.language),
        SortColumn::Lines => left.lines.cmp(&right.lines),
        SortColumn::Code => left.code.cmp(&right.code),
        SortColumn::Comments => left.comments.cmp(&right.comments),
        SortColumn::Blanks => left.blanks.cmp(&right.blanks),
    }
}
