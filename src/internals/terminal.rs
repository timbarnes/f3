use crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};
use std::io;

/// Enable raw mode for the terminal
pub fn enable_raw() -> io::Result<()> {
    enable_raw_mode()
}

/// Disable raw mode for the terminal
pub fn disable_raw() -> io::Result<()> {
    disable_raw_mode()
}

/// Check if raw mode is enabled
pub fn get_raw_mode() -> io::Result<bool> {
    is_raw_mode_enabled()
}
