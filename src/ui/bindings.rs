use crossterm::event::{KeyCode, KeyModifiers};

/// The single vocabulary of user actions. Keys map to these, and screens act
/// on them. This file is the one source of truth for keyboard commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Move the highlight up (or scroll up).
    MoveUp,
    /// Move the highlight down (or scroll down).
    MoveDown,
    /// Open the selected item or commit a text entry.
    Open,
    /// Load the highlighted recent path into the input.
    OpenRecent,
    /// Go back to the previous screen or clear a search.
    Back,
    /// Insert one typed character (text entry only).
    Type(char),
    /// Delete the last typed character (text entry only).
    EraseChar,
    /// Pick the previous column as the sort key.
    SortColumnLeft,
    /// Pick the next column as the sort key.
    SortColumnRight,
    /// Toggle the sort direction on the chosen column.
    OrderToggle,
    /// Begin editing the search term.
    StartSearch,
    /// Show the next result view.
    NextView,
    /// Show the previous result view.
    PreviousView,
    /// Jump 50 rows up.
    PageUp,
    /// Jump 50 rows down.
    PageDown,
    /// Exit the application.
    Quit,
}

/// Map a key to a [`Command`]. Keys not listed do nothing.
///
/// `Esc` is the single exit key. Plain arrows move and confirm. Holding Shift
/// with Left/Right picks the sort column. `Space` toggles the sort direction on
/// the chosen column. Tab and BackTab switch result views.
pub fn command_for_key(code: KeyCode, mods: KeyModifiers) -> Option<Command> {
    if code == KeyCode::Esc {
        return Some(Command::Quit);
    }
    let shifted = mods.contains(KeyModifiers::SHIFT);
    match (code, shifted) {
        (KeyCode::Up, _) => Some(Command::MoveUp),
        (KeyCode::Down, _) => Some(Command::MoveDown),
        (KeyCode::Right, false) => Some(Command::NextView),
        (KeyCode::Right, true) => Some(Command::SortColumnRight),
        (KeyCode::Left, false) => Some(Command::Back),
        (KeyCode::Left, true) => Some(Command::SortColumnLeft),
        (KeyCode::Char(' '), false) => Some(Command::OrderToggle),
        (KeyCode::Char('s'), false) => Some(Command::StartSearch),
        (KeyCode::Tab, _) => Some(Command::NextView),
        (KeyCode::BackTab, _) => Some(Command::PreviousView),
        (KeyCode::PageUp, _) => Some(Command::PageUp),
        (KeyCode::PageDown, _) => Some(Command::PageDown),
        _ => None,
    }
}

/// Map a key to a [`Command`] while the user is typing a repository path.
/// Esc still exits; every character is inserted as text. Up and Down move the
/// highlight over the recent paths, and Right loads the highlighted one into
/// the input.
pub fn command_for_text_key(code: KeyCode, _mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Esc => Some(Command::Quit),
        KeyCode::Char(c) if !c.is_control() => Some(Command::Type(c)),
        KeyCode::Backspace => Some(Command::EraseChar),
        KeyCode::Enter => Some(Command::Open),
        KeyCode::Up => Some(Command::MoveUp),
        KeyCode::Down => Some(Command::MoveDown),
        KeyCode::Right => Some(Command::OpenRecent),
        _ => None,
    }
}

/// Map a key to a [`Command`] while the user is typing a search term in the
/// file table. Characters edit the term live; Backspace removes a character;
/// Enter commits the search; Left clears the search and returns to the full
/// list. Esc still exits the application, matching every other screen.
pub fn command_for_search_key(code: KeyCode, _mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Esc => Some(Command::Quit),
        KeyCode::Char(c) if !c.is_control() => Some(Command::Type(c)),
        KeyCode::Backspace => Some(Command::EraseChar),
        KeyCode::Enter => Some(Command::Open),
        KeyCode::Left => Some(Command::Back),
        _ => None,
    }
}

/// Help text while typing a repository path.
pub fn help_for_picker_input() -> &'static str {
    "type a git repository path | Enter: scan | ↑/↓ then →: recent | Esc: exit"
}

/// Help text for the file table.
pub fn help_for_files() -> &'static str {
    "↑/↓: scroll | PgUp/PgDn: 50 | Shift+←/→: sort column | Space: direction | s: search | Tab: languages | ←: repository | Esc: exit"
}

/// Help text for the language summary.
pub fn help_for_languages() -> &'static str {
    "↑/↓: scroll | PgUp/PgDn: 50 | Tab: files | ←: repository | Esc: exit"
}
