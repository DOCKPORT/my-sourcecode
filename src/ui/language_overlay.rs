use crate::ui::Action;
use crate::ui::bindings::Command;
use crate::ui::table::{TableSpec, draw_table};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, TableState};

/// How many rows one page key jumps in the overlay.
const PAGE_JUMP: isize = 50;

/// A scrollable overlay with every language, including the rows hidden by the
/// height limit of the summary table.
pub struct LanguageOverlay {
    rows: Vec<Vec<String>>,
    colors: Vec<Option<Color>>,
    state: TableState,
}

impl LanguageOverlay {
    /// Build the overlay from all formatted language rows.
    pub fn new(rows: Vec<Vec<String>>, colors: Vec<Option<Color>>) -> Self {
        let mut state = TableState::default();
        state.select(if rows.is_empty() { None } else { Some(0) });
        Self {
            rows,
            colors,
            state,
        }
    }

    /// Handle one command from the overlay key map.
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
            Command::Back => Some(Action::CloseLanguages),
            Command::Quit => Some(Action::Quit),
            Command::Type(_)
            | Command::EraseChar
            | Command::Open
            | Command::OpenRecent
            | Command::SortColumnLeft
            | Command::SortColumnRight
            | Command::OrderToggle
            | Command::StartSearch
            | Command::OpenLanguages => None,
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

    /// Draw the overlay into the given area.
    pub fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Languages ({}) ", self.rows.len()));

        if self.rows.is_empty() {
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
                rows: self.rows.clone(),
                widths: widths.to_vec(),
                left_aligned: centered.to_vec(),
                row_rules: false,
                body_dividers: false,
                color_column: Some(0),
                cell_colors: self.colors.clone(),
            },
            Some(&mut self.state),
        );
    }
}
