//! Modular Ratatui terminal user interface (TUI) subsystem.
//!
//! Provides the interactive Accounts manager table and the Session history explorer,
//! guarded by RAII terminal state restoration preventing terminal corruption on exit or panic.

pub mod accounts;
pub mod sessions;
pub mod widgets;

pub use accounts::run_accounts_tui;
pub use sessions::run_sessions_tui;

use anyhow::Result;
use crossterm::{
    cursor::Show,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

/// Type alias for the standard Ratatui Crossterm terminal.
pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

/// RAII guard ensuring the terminal enters raw mode and alternate screen on creation,
/// and unconditionally restores normal cooked terminal state upon drop or panic.
pub struct TerminalGuard {
    terminal: TuiTerminal,
}

impl TerminalGuard {
    /// Initializes raw mode, enters alternate screen, and constructs a Ratatui terminal backend.
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Provides mutable access to the underlying Ratatui terminal for frame rendering.
    pub fn terminal_mut(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }

    /// Safely restores standard terminal mode (cooked mode, leave alternate screen, show cursor).
    pub fn restore() {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        Self::restore();
    }
}
