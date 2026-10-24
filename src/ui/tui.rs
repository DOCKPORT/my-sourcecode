use anyhow::Result;
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Stdout};

/// The concrete terminal backend used by this application.
pub type Backend = CrosstermBackend<Stdout>;
/// The ratatui terminal handle used for drawing.
pub type TuiTerminal = Terminal<Backend>;

/// Enter raw mode and the alternate screen, returning a ready terminal.
///
/// Call [`restore`] when done so the user's terminal is put back exactly as it
/// was before the application started.
pub fn init() -> Result<TuiTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen) {
        reset_terminal();
        return Err(error.into());
    }
    // Ask the terminal to report modified keys (for example Shift) precisely.
    // Terminals that support this report Shift+Left/Right; terminals that do
    // not ignore the request, so the app never relies on it for sorting.
    if let Err(error) = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    ) {
        reset_terminal();
        return Err(error.into());
    }
    let backend = CrosstermBackend::new(stdout);
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            reset_terminal();
            Err(error.into())
        }
    }
}

/// Undo terminal setup after a failed start. Each step runs even if an earlier
/// one fails, so the user's terminal is not left in raw or alternate mode.
fn reset_terminal() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, PopKeyboardEnhancementFlags);
    let _ = execute!(stdout, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// Leave the alternate screen and restore normal terminal behaviour.
pub fn restore(mut terminal: TuiTerminal) -> Result<()> {
    let mut first_error: Option<anyhow::Error> = None;
    if let Err(error) = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags) {
        first_error = Some(error.into());
    }
    if let Err(error) = execute!(terminal.backend_mut(), LeaveAlternateScreen)
        && first_error.is_none()
    {
        first_error = Some(error.into());
    }
    if let Err(error) = disable_raw_mode()
        && first_error.is_none()
    {
        first_error = Some(error.into());
    }
    if let Err(error) = terminal.show_cursor()
        && first_error.is_none()
    {
        first_error = Some(error.into());
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
