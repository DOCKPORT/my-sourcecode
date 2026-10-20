use crate::ui::Action;
use crate::ui::bindings::Command;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::path::Path;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Maximum file size the viewer loads.
const MAX_BYTES: u64 = 1024 * 1024;
/// The color theme used for syntax highlighting.
const THEME_NAME: &str = "base16-ocean.dark";

/// One styled part of a source line.
struct Segment {
    style: Style,
    text: String,
}

/// A read-only view of one file with line numbers and syntax highlighting.
pub struct FileView {
    path: String,
    segments: Vec<Vec<Segment>>,
    message: Option<String>,
    offset: usize,
    viewport: usize,
}

impl FileView {
    /// Load a file relative to the repository root. A load problem becomes a
    /// message inside the viewer, so the user can read it and go back.
    pub fn open(root: &Path, relative: &str, language: &str) -> Self {
        let full = root.join(relative);
        let mut view = Self {
            path: relative.to_string(),
            segments: Vec::new(),
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

        let mut lines: Vec<String> = text.lines().map(expand_tabs).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }

        view.segments =
            highlight(&lines, language, relative).unwrap_or_else(|| plain_segments(&lines));
        view
    }

    /// File path relative to the repository root.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Number of loaded lines.
    pub fn len(&self) -> usize {
        self.segments.len()
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
        let max = self.segments.len().saturating_sub(self.viewport.max(1));
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

        let number_width = self.segments.len().to_string().len();
        let visible: Vec<Line> = self
            .segments
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(inner.height as usize)
            .map(|(index, segments)| {
                let mut spans = vec![
                    Span::styled(
                        format!("{:>number_width$}", index + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" │ "),
                ];
                for segment in segments {
                    spans.push(Span::styled(segment.text.clone(), segment.style));
                }
                Line::from(spans)
            })
            .collect();

        frame.render_widget(Paragraph::new(Text::from(visible)), inner);
    }
}

/// Highlight all lines with syntect. Return None when the language is unknown.
fn highlight(lines: &[String], language: &str, path: &str) -> Option<Vec<Vec<Segment>>> {
    let syntax_set = syntax_set();
    let syntax = syntax_set
        .find_syntax_by_name(language)
        .or_else(|| {
            Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(|extension| syntax_set.find_syntax_by_extension(extension))
        })?;

    let mut highlighter = HighlightLines::new(syntax, theme());
    let mut output = Vec::with_capacity(lines.len());

    for line in lines {
        let mut source = String::with_capacity(line.len() + 1);
        source.push_str(line);
        source.push('\n');

        let ranges = highlighter.highlight_line(&source, syntax_set).ok()?;
        let mut segments = Vec::new();
        for (style, text) in ranges {
            let text = text.trim_end_matches(['\n', '\r']);
            if text.is_empty() {
                continue;
            }
            segments.push(Segment {
                style: convert_style(style),
                text: text.to_string(),
            });
        }
        output.push(segments);
    }

    Some(output)
}

fn plain_segments(lines: &[String]) -> Vec<Vec<Segment>> {
    lines
        .iter()
        .map(|line| {
            vec![Segment {
                style: Style::default(),
                text: line.clone(),
            }]
        })
        .collect()
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
