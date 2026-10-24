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
    /// Open the full language overlay.
    OpenLanguages,
    /// Jump 50 rows up.
    PageUp,
    /// Jump 50 rows down.
    PageDown,
    /// Exit the application.
    Quit,
}

/// Map a key to a [`Command`]. Keys not listed do nothing.
///
/// `Esc` is the single exit key. Arrows move and Enter confirms. Holding Shift
/// with Left/Right picks the sort column. `Space` toggles the sort direction on
/// the chosen column.
pub fn command_for_key(code: KeyCode, mods: KeyModifiers) -> Option<Command> {
    if code == KeyCode::Esc {
        return Some(Command::Quit);
    }
    let shifted = mods.contains(KeyModifiers::SHIFT);
    match (code, shifted) {
        (KeyCode::Up, _) => Some(Command::MoveUp),
        (KeyCode::Down, _) => Some(Command::MoveDown),
        (KeyCode::Enter, _) => Some(Command::Open),
        (KeyCode::Left, false) => Some(Command::Back),
        (KeyCode::Left, true) => Some(Command::SortColumnLeft),
        (KeyCode::Right, true) => Some(Command::SortColumnRight),
        (KeyCode::Char(' '), false) => Some(Command::OrderToggle),
        (KeyCode::Char('s'), false) => Some(Command::StartSearch),
        (KeyCode::Char('l' | 'L'), _) => Some(Command::OpenLanguages),
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

/// Map a key to a [`Command`] while the file viewer is open.
///
/// Left closes the viewer. Up/Down and the page keys scroll the file. Esc does
/// nothing here, so the user must close the viewer before exiting.
pub fn command_for_file_view_key(code: KeyCode, _mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Left => Some(Command::Back),
        KeyCode::Up => Some(Command::MoveUp),
        KeyCode::Down => Some(Command::MoveDown),
        KeyCode::PageUp => Some(Command::PageUp),
        KeyCode::PageDown => Some(Command::PageDown),
        _ => None,
    }
}

/// Map a key to a [`Command`] while the language overlay is open.
///
/// `L` closes the overlay. The page keys scroll the list. Esc does nothing while
/// the overlay is open, so the user must close it before exiting.
pub fn command_for_overlay_key(code: KeyCode, _mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Char('l') | KeyCode::Char('L') => Some(Command::Back),
        KeyCode::Up => Some(Command::MoveUp),
        KeyCode::Down => Some(Command::MoveDown),
        KeyCode::PageUp => Some(Command::PageUp),
        KeyCode::PageDown => Some(Command::PageDown),
        _ => None,
    }
}

/// Map a key to a [`Command`] while the user is typing a search term in the
/// file table. The filter applies live as characters arrive; Backspace removes
/// a character; Esc clears the search and leaves search mode. Enter does
/// nothing here. The application keeps running, so a second Esc exits from the
/// normal map.
pub fn command_for_search_key(code: KeyCode, _mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Esc => Some(Command::Back),
        KeyCode::Char(c) if !c.is_control() => Some(Command::Type(c)),
        KeyCode::Backspace => Some(Command::EraseChar),
        _ => None,
    }
}

/// Help text while typing a repository path.
pub fn help_for_picker_input() -> &'static str {
    "type a git repository path | Enter: open | ↑/↓: recent | Esc: exit"
}

/// Help text for the file viewer.
pub fn help_for_file_view() -> &'static str {
    "↑/↓: scroll | PgUp/PgDn: page | ←: close file"
}

/// Help text for the language overlay.
pub fn help_for_overlay() -> &'static str {
    "↑/↓: scroll | PgUp/PgDn: 50 | L: close all languages"
}

/// Help text for the file table.
pub fn help_for_files() -> &'static str {
    "↑/↓: scroll | Enter: view file | PgUp/PgDn: 50 | Shift+←/→: sort | Space: direction | s: search | L: all languages | ←: home | Esc: exit"
}
