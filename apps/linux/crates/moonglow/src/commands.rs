//! Shared state handle and the commands the UI thread must perform itself.
//!
//! State mutations happen directly on the shared [`StateManager`]. Only work
//! that the event loop owns — revealing the window or quitting — travels over
//! this channel.

use moonglow_core::state::StateManager;
use std::sync::{Arc, Mutex};

/// Commands the event loop owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    /// Reveal and focus the settings window.
    ShowWindow,
    /// Run the shutdown bundle and exit.
    Quit,
}

pub type SharedState<S> = Arc<Mutex<StateManager<S>>>;
