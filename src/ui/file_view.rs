use crate::ui::Action;
use crate::ui::bindings::Command;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// The color theme used for syntax highlighting.
const THEME_NAME: &str = "base16-eighties.dark";

/// A read-only view of one file with line numbers and lazy syntax highlighting.
pub struct FileView {
    path: String,
    file: Option<File>,
    /// Byte offset of each line start.
    offsets: Vec<usize>,
    file_len: usize,
    syntax: Option<&'static SyntaxReference>,
    message: Option<String>,
    offset: usize,
    viewport: usize,
}

impl FileView {
    /// Load a file relative to the repository root. A load problem becomes a
    /// message inside the viewer, so the user can read it and go back.
    ///
    /// Syntax parsing is lazy. Only visible lines are parsed, so large files
    /// open quickly.
    pub fn open(root: &Path, relative: &str, language: &str) -> Self {
        let full = root.join(relative);
        let mut view = Self {
            path: relative.to_string(),
            file: None,
            offsets: Vec::new(),
            file_len: 0,
            syntax: find_syntax(language, relative),
            message: None,
            offset: 0,
            viewport: 1,
        };

        let file = match File::open(&full) {
            Ok(file) => file,
            Err(error) => {
                view.message = Some(format!("Cannot open {}: {error}", full.display()));
                return view;
            }
        };
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                view.message = Some(format!("Cannot read {}: {error}", full.display()));
                return view;
            }
        };
        if !metadata.is_file() {
            view.message = Some("Cannot view a directory.".to_string());
            return view;
        }

        view.file_len = metadata.len() as usize;
        let mut reader = BufReader::new(file);
        let mut offsets = vec![0usize];
        let mut position = 0usize;
        let mut line = Vec::new();

        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(read) => {
                    if line.contains(&0) {
                        view.message = Some("Cannot show a binary file.".to_string());
                        return view;
                    }
                    if std::str::from_utf8(&line).is_err() {
                        view.message =
                            Some("Cannot show a file that is not valid UTF-8.".to_string());
                        return view;
                    }
                    position = position.saturating_add(read);
                    if line.last() == Some(&b'\n') {
                        offsets.push(position);
                    }
                }
                Err(error) => {
                    view.message = Some(format!("Cannot read {}: {error}", full.display()));
                    return view;
                }
            }
        }

        if position > 0 && offsets.last() == Some(&position) {
            offsets.pop();
        }
        if offsets.is_empty() {
            offsets.push(0);
        }

        view.offsets = offsets;
        view.file = Some(reader.into_inner());
        view
    }

    /// File path relative to the repository root.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Number of lines in the file.
    pub fn len(&self) -> usize {
        self.offsets.len()
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
        let max = self.offsets.len().saturating_sub(self.viewport.max(1));
        self.offset = offset.min(max);
    }

    /// Read one range of whole lines from the file.
    fn read_window(&mut self, start: usize, end: usize) -> Result<Vec<String>, String> {
        if start >= end {
            return Ok(Vec::new());
        }
        let start_byte = self.offsets[start];
        let end_byte = if end < self.offsets.len() {
            self.offsets[end]
        } else {
            self.file_len
        };
        if end_byte < start_byte {
            return Err("The file changed while it was open.".to_string());
        }

        let Some(file) = self.file.as_mut() else {
            return Err("The file is not available.".to_string());
        };
        let mut bytes = vec![0u8; end_byte - start_byte];
        file.seek(SeekFrom::Start(start_byte as u64))
            .map_err(|error| format!("Cannot read {}: {error}", self.path))?;
        file.read_exact(&mut bytes)
            .map_err(|error| format!("Cannot read {}: {error}", self.path))?;
        let text =
            String::from_utf8(bytes).map_err(|_| "The file is not valid UTF-8.".to_string())?;

        let expected = end - start;
        let mut lines: Vec<String> = text
            .split('\n')
            .map(|line| expand_tabs(line.trim_end_matches('\r')))
            .collect();
        if text.ends_with('\n') {
            lines.pop();
        }
        lines.resize(expected, String::new());
        Ok(lines)
    }

    /// Draw the viewer into the given area.
    pub fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.path))
            .style(Style::default().bg(theme::BACKGROUND))
            .border_style(Style::default().fg(theme::BORDER))
            .title_style(Style::default().fg(theme::FOREGROUND));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.viewport = inner.height.max(1) as usize;
        self.scroll_to(self.offset);

        if let Some(message) = &self.message {
            render_message(frame, inner, message);
            return;
        }

        let number_width = self.offsets.len().to_string().len();
        let end = self
            .offset
            .saturating_add(inner.height as usize)
            .min(self.offsets.len());
        let lines = match self.read_window(self.offset, end) {
            Ok(lines) => lines,
            Err(error) => {
                self.message = Some(error.clone());
                render_message(frame, inner, &error);
                return;
            }
        };

        let syntax_set = syntax_set();
        let mut highlighter = self
            .syntax
            .map(|syntax| HighlightLines::new(syntax, theme()));
        let visible: Vec<Line> = lines
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let mut spans = vec![
                    Span::styled(
                        format!("{:>number_width$}", self.offset + index + 1),
                        Style::default().fg(theme::MUTED),
                    ),
                    Span::raw(" │ "),
                ];
                let start = spans.len();
                if let Some(highlighter) = highlighter.as_mut() {
                    let mut source = String::with_capacity(text.len() + 1);
                    source.push_str(&text);
                    source.push('\n');
                    if let Ok(ranges) = highlighter.highlight_line(&source, syntax_set) {
                        for (style, part) in ranges {
                            let part = part.trim_end_matches(['\n', '\r']);
                            if !part.is_empty() {
                                spans.push(Span::styled(part.to_string(), convert_style(style)));
                            }
                        }
                    }
                }
                if spans.len() == start {
                    spans.push(Span::raw(text));
                }
                Line::from(spans)
            })
            .collect();

        frame.render_widget(
            Paragraph::new(Text::from(visible))
                .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND)),
            inner,
        );
    }
}

/// Draw a plain message inside the viewer.
fn render_message(frame: &mut Frame<'_>, area: Rect, message: &str) {
    let paragraph = Paragraph::new(Text::raw(message.to_string()))
        .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn find_syntax(language: &str, path: &str) -> Option<&'static syntect::parsing::SyntaxReference> {
    let syntax_set = syntax_set();
    syntax_set.find_syntax_by_name(language).or_else(|| {
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(|extension| syntax_set.find_syntax_by_extension(extension))
    })
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();
    let themes = THEMES.get_or_init(ThemeSet::load_defaults);
    themes
        .themes
        .get(THEME_NAME)
        .or_else(|| themes.themes.values().next())
        .expect("syntect default themes")
}

fn convert_style(style: syntect::highlighting::Style) -> Style {
    let foreground = style.foreground;
    let mut result = Style::default().fg(Color::Rgb(foreground.r, foreground.g, foreground.b));
    if style.font_style.contains(FontStyle::BOLD) {
        result = result.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        result = result.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        result = result.add_modifier(Modifier::UNDERLINED);
    }
    result
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
