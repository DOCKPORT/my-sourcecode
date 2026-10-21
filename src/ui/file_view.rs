use crate::ui::Action;
use crate::ui::bindings::Command;
use crate::ui::theme;
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
const THEME_NAME: &str = "base16-eighties.dark";

/// One styled part of a source line.
#[derive(Clone)]
struct Segment {
    style: Style,
    text: String,
}

/// A read-only view of one file with line numbers and lazy syntax highlighting.
pub struct FileView {
    path: String,
    lines: Vec<String>,
    /// Cached highlights. None means the line has not been parsed yet.
    segments: Vec<Option<Vec<Segment>>>,
    highlighter: Option<HighlightLines<'static>>,
    /// Index of the next line to parse. All earlier lines are cached.
    highlighted_until: usize,
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
            lines: Vec::new(),
            segments: Vec::new(),
            highlighter: None,
            highlighted_until: 0,
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
        view.segments = vec![None; view.lines.len()];
        view.highlighter =
            find_syntax(language, relative).map(|syntax| HighlightLines::new(syntax, theme()));
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

    /// Parse lines until `end` lines are cached. Parsing must run in order,
    /// because syntect keeps state between lines.
    fn highlight_until(&mut self, end: usize) {
        let Some(highlighter) = self.highlighter.as_mut() else {
            return;
        };
        let syntax_set = syntax_set();
        let end = end.min(self.lines.len());

        while self.highlighted_until < end {
            let index = self.highlighted_until;
            let mut source = String::with_capacity(self.lines[index].len() + 1);
            source.push_str(&self.lines[index]);
            source.push('\n');

            let segments = match highlighter.highlight_line(&source, syntax_set) {
                Ok(ranges) => ranges
                    .into_iter()
                    .filter_map(|(style, text)| {
                        let text = text.trim_end_matches(['\n', '\r']);
                        if text.is_empty() {
                            None
                        } else {
                            Some(Segment {
                                style: convert_style(style),
                                text: text.to_string(),
                            })
                        }
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };

            self.segments[index] = Some(segments);
            self.highlighted_until += 1;
        }
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
        self.highlight_until(self.offset.saturating_add(self.viewport));

        if let Some(message) = &self.message {
            let paragraph = Paragraph::new(Text::raw(message.clone()))
                .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND))
                .wrap(Wrap { trim: false });
            frame.render_widget(paragraph, inner);
            return;
        }

        let number_width = self.lines.len().to_string().len();
        let end = self
            .offset
            .saturating_add(inner.height as usize)
            .min(self.lines.len());
        let visible: Vec<Line> = (self.offset..end)
            .map(|index| {
                let mut spans = vec![
                    Span::styled(
                        format!("{:>number_width$}", index + 1),
                        Style::default().fg(theme::MUTED),
                    ),
                    Span::raw(" │ "),
                ];
                match self.segments.get(index).and_then(|line| line.as_ref()) {
                    Some(segments) => {
                        for segment in segments {
                            spans.push(Span::styled(segment.text.clone(), segment.style));
                        }
                    }
                    None => spans.push(Span::raw(self.lines[index].clone())),
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
