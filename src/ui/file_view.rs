use crate::ui::Action;
use crate::ui::bindings::Command;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::path::Path;

/// Maximum file size the viewer loads.
const MAX_BYTES: u64 = 1024 * 1024;

/// A read-only view of one file with line numbers and vertical scrolling.
pub struct FileView {
    path: String,
    lines: Vec<String>,
    message: Option<String>,
    offset: usize,
    viewport: usize,
}

impl FileView {
    /// Load a file relative to the repository root. A load problem becomes a
    /// message inside the viewer, so the user can read it and go back.
    pub fn open(root: &Path, relative: &str) -> Self {
        let full = root.join(relative);
        let mut view = Self {
            path: relative.to_string(),
            lines: Vec::new(),
            message: None,
            offset: 0,
            viewport: 1,
        };

        let metadata = match std::fs::metadata(&full) {
            Ok(metadata) => metadata,
            Err(error) => {
                view.message = Some(format!("Cannot open {}: {error}", full.display()));
                return view;
            }
        };

        if metadata.len() > MAX_BYTES {
            view.message = Some(format!(
                "File is too large to view: {} bytes. The limit is 1 MiB.",
                metadata.len()
            ));
            return view;
        }

        let bytes = match std::fs::read(&full) {
            Ok(bytes) => bytes,
            Err(error) => {
                view.message = Some(format!("Cannot read {}: {error}", full.display()));
                return view;
            }
        };

        if bytes.contains(&0) {
            view.message = Some("Cannot show a binary file.".to_string());
            return view;
        }

        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                view.message = Some("Cannot show a file that is not valid UTF-8.".to_string());
                return view;
            }
        };

        view.lines = text.lines().map(expand_tabs).collect();
        if view.lines.is_empty() {
            view.lines.push(String::new());
        }
        view
    }

    /// File path relative to the repository root.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Number of loaded lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// One-based index of the top visible line.
    pub fn top_line(&self) -> usize {
        self.offset + 1
    }

    /// Handle one command from the viewer key map.
    pub fn handle(&mut self, command: Command) -> Option<Action> {
        match command {
            Command::MoveUp => {
                self.offset = self.offset.saturating_sub(1);
                None
            }
            Command::MoveDown => {
                self.scroll_to(self.offset.saturating_add(1));
                None
            }
            Command::PageUp => {
                self.scroll_to(self.offset.saturating_sub(self.viewport));
                None
            }
            Command::PageDown => {
                self.scroll_to(self.offset.saturating_add(self.viewport));
                None
            }
            Command::Back => Some(Action::CloseFile),
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

    fn scroll_to(&mut self, offset: usize) {
        let max = self.lines.len().saturating_sub(self.viewport.max(1));
        self.offset = offset.min(max);
    }

    /// Draw the viewer into the given area.
    pub fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.viewport = inner.height.max(1) as usize;
        self.scroll_to(self.offset);

        if let Some(message) = &self.message {
            let paragraph = Paragraph::new(Text::raw(message.clone())).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, inner);
            return;
        }

        let number_width = self.lines.len().to_string().len();
        let visible: Vec<Line> = self
            .lines
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(inner.height as usize)
            .map(|(index, line)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:>number_width$}", index + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" │ "),
                    Span::raw(line.clone()),
                ])
            })
            .collect();

        frame.render_widget(Paragraph::new(Text::from(visible)), inner);
    }
}

/// Expand tabs to four spaces for stable alignment.
fn expand_tabs(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    for character in line.chars() {
        match character {
            '\t' => output.push_str("    "),
            '\r' => {}
            other => output.push(other),
        }
    }
    output
}
