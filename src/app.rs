use crate::scan::{self, Report};
use crate::ui::Action;
use crate::ui::bindings::{self, Command};
use crate::ui::file_view::FileView;
use crate::ui::files::FileTable;
use crate::ui::git::GitSummary;
use crate::ui::language_overlay::LanguageOverlay;
use crate::ui::languages::LanguageSummary;
use crate::ui::picker::Picker;
use crate::ui::summary::Summary;
use crate::ui::theme;
use crate::ui::tui::{self, TuiTerminal};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, Paragraph};
use std::path::Path;

/// The application shell. Before a scan it shows the repository picker; after
/// a scan it shows the totals, the language summary, and the file table.
pub struct App {
    /// The repository picker, always available.
    picker: Picker,
    /// Display path of the scanned repository.
    root: Option<String>,
    /// Repository totals shown above the tables.
    summary: Option<Summary>,
    /// The file table of the scanned repository.
    files: Option<FileTable>,
    /// The language summary of the scanned repository.
    languages: Option<LanguageSummary>,
    /// The repository information table of the scanned repository.
    git: Option<GitSummary>,
    /// The full language list overlay, while it is open.
    languages_overlay: Option<LanguageOverlay>,
    /// The read-only file viewer, while it is open.
    file_view: Option<FileView>,
    should_quit: bool,
}

impl App {
    /// Build the application shell, starting at the repository picker.
    pub fn new() -> Self {
        Self {
            picker: Picker::new(),
            root: None,
            summary: None,
            files: None,
            languages: None,
            git: None,
            languages_overlay: None,
            file_view: None,
            should_quit: false,
        }
    }

    /// Scan `path` and show its results. A failure is reported in the picker so
    /// the user can correct the path without losing the session.
    ///
    /// The path is made absolute first. A relative argument such as `.` must
    /// not enter the recent list as a relative entry.
    pub fn open_repository(&mut self, path: &Path) {
        let root = match std::fs::canonicalize(path) {
            Ok(root) => root,
            Err(error) => {
                self.picker
                    .report(format!("Cannot open {}: {error}", path.display()));
                return;
            }
        };

        match scan::scan(&root) {
            Ok(report) => self.enter_results(report),
            Err(error) => self
                .picker
                .report(format!("Cannot scan {}: {error}", root.display())),
        }
    }

    /// Build the result screens from a finished scan.
    fn enter_results(&mut self, report: Report) {
        let summary = Summary::new(&report.files, &report.languages);
        let Report {
            root,
            files,
            languages,
            git,
        } = report;
        let display = root.display().to_string();

        self.picker.record_recent(&display);
        self.summary = Some(summary);
        self.files = Some(FileTable::new(files));
        self.languages = Some(LanguageSummary::new(languages));
        self.git = Some(GitSummary::new(git));
        self.languages_overlay = None;
        self.file_view = None;
        self.root = Some(display);
    }

    /// Run the draw and input loop until the user quits, then restore the
    /// terminal.
    pub fn run(&mut self, mut terminal: TuiTerminal) -> Result<()> {
        let result = self.run_loop(&mut terminal);
        let restore = tui::restore(terminal);
        match (result, restore) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
        }
    }
}

impl App {
    fn run_loop(&mut self, terminal: &mut TuiTerminal) -> Result<()> {
        // Draw once, then redraw only when an event actually changed something.
        // This avoids rebuilding the frame for ignored events such as dropped
        // Ctrl and Alt keys or stray mouse moves.
        let mut needs_redraw = true;
        while !self.should_quit {
            if needs_redraw {
                terminal.draw(|frame| self.render(frame))?;
            }
            needs_redraw = self.handle_events()?;
        }
        Ok(())
    }

    /// Read one input event, map it to a command, and dispatch to the active
    /// screen. Return whether the frame changed and must be redrawn.
    fn handle_events(&mut self) -> Result<bool> {
        match event::read()? {
            // A size change requires a full redraw to re-layout the screens.
            Event::Resize(_, _) => Ok(true),
            Event::Key(key) => {
                // Accept plain keys and shifted text. Drop Ctrl and Alt
                // combinations so that only ordinary typing and navigation
                // reach the screens.
                if key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    return Ok(false);
                }

                let Some(command) = self.map_command(key.code, key.modifiers) else {
                    return Ok(false);
                };

                let action = if let Some(view) = self.file_view.as_mut() {
                    view.handle(command)
                } else if let Some(overlay) = self.languages_overlay.as_mut() {
                    overlay.handle(command)
                } else if self.files.is_none() {
                    self.picker.handle(command)
                } else {
                    self.files.as_mut().and_then(|files| files.handle(command))
                };

                self.apply(action);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Choose which key mapping applies. While the picker accepts a typed path,
    /// characters and editing keys use the text mapping. While the file table
    /// edits a search term, characters refine the query instead of running
    /// commands.
    fn map_command(&self, code: KeyCode, mods: KeyModifiers) -> Option<Command> {
        if self.file_view.is_some() {
            return bindings::command_for_file_view_key(code, mods);
        }
        if self.languages_overlay.is_some() {
            return bindings::command_for_overlay_key(code, mods);
        }
        if self.files.is_none() && self.picker.is_typing() {
            return bindings::command_for_text_key(code, mods);
        }
        if let Some(files) = self.files.as_ref()
            && files.searching()
        {
            return bindings::command_for_search_key(code, mods);
        }
        bindings::command_for_key(code, mods)
    }

    /// Carry out an action returned by the active screen.
    fn apply(&mut self, action: Option<Action>) {
        let Some(action) = action else {
            return;
        };
        match action {
            Action::Quit => self.should_quit = true,
            Action::ScanRepository(path) => self.open_repository(Path::new(&path)),
            Action::BackToPicker => {
                self.root = None;
                self.summary = None;
                self.files = None;
                self.languages = None;
                self.git = None;
                self.languages_overlay = None;
                self.file_view = None;
            }
            Action::ShowLanguages => {
                if let Some(languages) = self.languages.as_ref() {
                    self.languages_overlay = Some(LanguageOverlay::new(
                        languages.display_rows(),
                        languages.display_colors(),
                    ));
                }
            }
            Action::CloseLanguages => self.languages_overlay = None,
            Action::OpenFile { path, language } => {
                if let Some(root) = self.root.as_deref() {
                    self.file_view = Some(FileView::open(Path::new(root), &path, &language));
                }
            }
            Action::CloseFile => self.file_view = None,
        }
    }
}

impl App {
    /// Draw the current frame.
    fn render(&mut self, frame: &mut Frame<'_>) {
        frame.render_widget(
            Block::default().style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND)),
            frame.size(),
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(frame.size());

        let header = match &self.root {
            Some(root) => format!(
                " my-sourcecode v{}  {}  {} files",
                env!("CARGO_PKG_VERSION"),
                root,
                self.files.as_ref().map_or(0, FileTable::total)
            ),
            None => format!(" My-Sourcecode v{}", env!("CARGO_PKG_VERSION")),
        };
        frame.render_widget(
            Paragraph::new(Text::raw(header))
                .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND)),
            chunks[0],
        );

        if self.files.is_none() {
            self.picker.draw(frame, chunks[1]);
        } else if let Some(view) = self.file_view.as_mut() {
            view.draw(frame, chunks[1]);
        } else {
            let language_count = self.languages.as_ref().map_or(0, LanguageSummary::len);
            let language_height = language_area_height(chunks[1].height, language_count);
            let areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(language_height),
                    Constraint::Length(5),
                    Constraint::Min(5),
                ])
                .split(chunks[1]);

            if let Some(summary) = self.summary.as_ref() {
                summary.draw(frame, areas[0]);
            }
            if let Some(languages) = self.languages.as_ref() {
                languages.draw(frame, areas[1]);
            }
            if let Some(git) = self.git.as_ref() {
                git.draw(frame, areas[2]);
            }
            if let Some(files) = self.files.as_mut() {
                files.draw(frame, areas[3]);
            }
        }

        if let Some(overlay) = self.languages_overlay.as_mut() {
            let screen = frame.size();
            let width = screen.width * 8 / 10;
            let height = screen.height * 8 / 10;
            let area = Rect {
                x: screen.x + screen.width.saturating_sub(width) / 2,
                y: screen.y + screen.height.saturating_sub(height) / 2,
                width,
                height,
            };
            overlay.draw(frame, area);
        }

        let (footer, cursor_x) = self.footer();
        frame.render_widget(
            Paragraph::new(Text::raw(footer))
                .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND)),
            chunks[2],
        );
        if let Some(x) = cursor_x {
            let x = x.min(chunks[2].right().saturating_sub(1));
            frame.set_cursor(x, chunks[2].y);
        }
    }

    /// The footer text and, while a search term is typed, the cursor column.
    fn footer(&self) -> (String, Option<u16>) {
        if let Some(view) = self.file_view.as_ref() {
            return (
                format!(
                    " {} | line {}/{} | {} ",
                    view.path(),
                    view.top_line(),
                    view.len(),
                    bindings::help_for_file_view()
                ),
                None,
            );
        }
        if self.languages_overlay.is_some() {
            return (format!(" {} ", bindings::help_for_overlay()), None);
        }
        if self.files.is_none() {
            return (format!(" {} ", self.picker.footer_help()), None);
        }

        let Some(files) = self.files.as_ref() else {
            return (String::new(), None);
        };

        if files.searching() {
            let prefix = " /search: ";
            let term = files.search_term();
            let x = (prefix.chars().count() + term.chars().count()) as u16;
            return (format!("{prefix}{term} | Esc: cancel"), Some(x));
        }

        (
            format!(
                " {}/{} files | {} ",
                files.cursor(),
                files.shown(),
                bindings::help_for_files()
            ),
            None,
        )
    }
}

/// Height for the language summary.
///
/// Each language uses two lines because one blank line sits between entries.
/// The header, the rule, and the borders add three lines. The summary uses at
/// most one third of the content area and at most ten languages, so the file
/// table always keeps useful space.
fn language_area_height(content_height: u16, languages: usize) -> u16 {
    if languages == 0 {
        return 3;
    }
    let budget = (content_height / 3).max(5);
    let max_entries = (budget.saturating_sub(3) / 2).max(1);
    let rows = (languages as u16).min(10).min(max_entries);
    rows * 2 + 3
}
