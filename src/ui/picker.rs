use crate::store::RecentPaths;
use crate::ui::Action;
use crate::ui::bindings::{self, Command};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::path::PathBuf;

/// The opening screen: type a repository path, or pick a recent one.
pub struct Picker {
    /// The path the user typed.
    input: String,
    /// Guidance, errors, or the last scan result.
    status: Option<String>,
    /// The recent repositories, loaded from disk and saved on each scan.
    recents: RecentPaths,
    /// Index of the highlighted recent path, when one is selected.
    recent_selected: Option<usize>,
}

impl Picker {
    /// Build a fresh picker.
    pub fn new() -> Self {
        Self {
            input: String::new(),
            status: Some("Enter the path to a local git repository, then press Enter.".to_string()),
            recents: RecentPaths::load(),
            recent_selected: None,
        }
    }

    /// True while the user is editing the path. The picker has no other mode.
    pub fn is_typing(&self) -> bool {
        true
    }

    /// Help text shown in the app footer.
    pub fn footer_help(&self) -> &'static str {
        bindings::help_for_picker_input()
    }

    /// Show a message, for example when a scan fails.
    pub fn report(&mut self, message: String) {
        self.status = Some(message);
    }

    /// Save a successfully scanned repository to the recent list.
    pub fn record_recent(&mut self, path: &str) {
        self.recents.add(path);
    }

    /// Handle one command. Returns an action only when the command leaves this
    /// screen. Key-to-command mapping lives in the bindings module.
    pub fn handle(&mut self, command: Command) -> Option<Action> {
        match command {
            Command::Type(ch) => {
                self.input.push(ch);
                self.status = None;
                self.recent_selected = None;
                None
            }
            Command::EraseChar => {
                self.input.pop();
                self.status = None;
                self.recent_selected = None;
                None
            }
            Command::Open => self.submit(),
            Command::OpenRecent => {
                if let Some(index) = self.recent_selected
                    && let Some(path) = self.recents.entries().get(index)
                {
                    self.input = path.clone();
                    self.recent_selected = None;
                    self.status = Some("Press Enter to scan this repository.".to_string());
                }
                None
            }
            Command::MoveUp => {
                self.move_recent(-1);
                None
            }
            Command::MoveDown => {
                self.move_recent(1);
                None
            }
            Command::Quit => Some(Action::Quit),
            Command::Back
            | Command::SortColumnLeft
            | Command::SortColumnRight
            | Command::OrderToggle
            | Command::StartSearch
            | Command::OpenLanguages
            | Command::PageUp
            | Command::PageDown => None,
        }
    }
}

impl Picker {
    /// Resolve the input path and ask the app to scan it.
    fn submit(&mut self) -> Option<Action> {
        let raw = if let Some(index) = self.recent_selected {
            self.recents.entries().get(index).cloned()
        } else if !self.input.trim().is_empty() {
            Some(self.input.clone())
        } else {
            None
        };

        let Some(raw) = raw else {
            self.status = Some("Enter a path first.".to_string());
            return None;
        };

        let path = expand_tilde(raw.trim());
        let root = match std::fs::canonicalize(&path) {
            Ok(root) => root,
            Err(error) => {
                self.status = Some(format!("Cannot open {}: {error}", path.display()));
                return None;
            }
        };

        if !root.is_dir() {
            self.status = Some(format!("Not a directory: {}", root.display()));
            return None;
        }

        self.input = root.display().to_string();
        self.recent_selected = None;
        Some(Action::ScanRepository(root.display().to_string()))
    }

    /// Move the highlight over the recent paths.
    fn move_recent(&mut self, delta: isize) {
        let count = self.recents.entries().len();
        if count == 0 {
            return;
        }
        let next = match self.recent_selected {
            Some(index) => (index as isize + delta).clamp(0, count as isize - 1) as usize,
            None => 0,
        };
        self.recent_selected = Some(next);
    }

    /// Draw the picker into the given area. The text cursor is parked at the
    /// end of the typed path while the input is visible.
    pub fn draw(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let status = self.status.clone().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(Text::raw(status)).style(Style::default().fg(Color::DarkGray)),
            chunks[0],
        );

        let input = Paragraph::new(Text::raw(self.input.clone())).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Git repository "),
        );
        frame.render_widget(input, chunks[1]);

        let width = chunks[1].width.saturating_sub(2) as usize;
        let typed = self.input.chars().count().min(width);
        let x = chunks[1].x + 1 + typed as u16;
        frame.set_cursor(x.min(chunks[1].right().saturating_sub(1)), chunks[1].y + 1);

        let items: Vec<ListItem> = self
            .recents
            .entries()
            .iter()
            .map(|path| ListItem::new(Text::raw(path.clone())))
            .collect();

        let title = if items.is_empty() {
            " Recent (none yet) "
        } else {
            " Recent "
        };
        let list = List::new(items)
            .highlight_symbol("> ")
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL).title(title));

        let mut state = ListState::default();
        state.select(self.recent_selected);
        frame.render_stateful_widget(list, chunks[2], &mut state);
    }
}

/// Expand a leading `~` or `~/` to the home directory.
fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }

    PathBuf::from(raw)
}
