//! Settings-window visibility state machine.
//!
//! Closing the window hides it instead of quitting, so the schedule and global
//! shortcuts keep running in the background; the window manager has its own
//! ideas (minimize, dock restore, taskbar). Sending `Visible` every frame
//! fights the compositor, and a dock/taskbar restore while we believe we are
//! hidden leaves the window stuck: mapped by the WM with no content (the black
//! page).
//!
//! This controller owns the reconciliation: commands go out only on change and
//! any OS-side restore signal (focus, input) while hidden re-shows.

/// Viewport effects, applied by the egui shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisCommand {
    SetVisible(bool),
    SetMinimized(bool),
    Focus,
    CancelClose,
}

pub struct VisibilityController {
    visible: bool,
    sent_visible: Option<bool>,
    sent_minimized: Option<bool>,
}

impl VisibilityController {
    /// `initially_visible` mirrors the viewport builder, so no command is
    /// emitted for the state the window is created in.
    pub fn new(initially_visible: bool) -> Self {
        Self {
            visible: initially_visible,
            sent_visible: Some(initially_visible),
            sent_minimized: None,
        }
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Show requested (IPC or launcher): reveal, unminimize, focus.
    pub fn show(&mut self) -> Vec<VisCommand> {
        self.visible = true;
        let mut cmds = self.emit_visible(true);
        cmds.extend(self.emit_minimized(false));
        cmds.push(VisCommand::Focus);
        cmds
    }

    /// The OS asked to close the window: always hide instead. Quit bypasses
    /// the event loop entirely (see `crate::quit_now`), so this never quits.
    pub fn on_close_requested(&mut self) -> Vec<VisCommand> {
        self.visible = false;
        let mut cmds = vec![VisCommand::CancelClose];
        cmds.extend(self.emit_visible(false));
        cmds
    }

    /// The OS restored or focused the window while we believed it hidden
    /// (dock/taskbar restore, Alt-Tab to a hidden window): adopt the truth
    /// and re-assert a visible, unminimized window with content.
    pub fn on_os_restore_signal(&mut self) -> Vec<VisCommand> {
        if self.visible {
            return Vec::new();
        }
        self.show()
    }

    fn emit_visible(&mut self, value: bool) -> Vec<VisCommand> {
        if self.sent_visible == Some(value) {
            return Vec::new();
        }
        self.sent_visible = Some(value);
        vec![VisCommand::SetVisible(value)]
    }

    fn emit_minimized(&mut self, value: bool) -> Vec<VisCommand> {
        if self.sent_minimized == Some(value) {
            return Vec::new();
        }
        self.sent_minimized = Some(value);
        vec![VisCommand::SetMinimized(value)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_reveals_unminimizes_and_focuses() {
        let mut vis = VisibilityController::new(false);
        assert_eq!(
            vis.show(),
            vec![
                VisCommand::SetVisible(true),
                VisCommand::SetMinimized(false),
                VisCommand::Focus,
            ]
        );
        assert!(vis.visible());
    }

    #[test]
    fn repeated_show_sends_no_duplicate_commands() {
        let mut vis = VisibilityController::new(true);
        // Already visible and unminimized-unknown: first show only unminimizes.
        assert_eq!(
            vis.show(),
            vec![VisCommand::SetMinimized(false), VisCommand::Focus]
        );
        // Steady state: nothing but the harmless focus nudge.
        assert_eq!(vis.show(), vec![VisCommand::Focus]);
    }

    #[test]
    fn close_hides_instead_of_quitting() {
        let mut vis = VisibilityController::new(true);
        assert_eq!(
            vis.on_close_requested(),
            vec![VisCommand::CancelClose, VisCommand::SetVisible(false)]
        );
        assert!(!vis.visible());
    }

    #[test]
    fn os_restore_while_hidden_reshows_with_content() {
        // The black-page regression: dock restore while we believed hidden.
        let mut vis = VisibilityController::new(true);
        vis.on_close_requested();
        assert!(!vis.visible());
        let cmds = vis.on_os_restore_signal();
        assert!(cmds.contains(&VisCommand::SetVisible(true)));
        assert!(cmds.contains(&VisCommand::SetMinimized(false)));
        assert!(vis.visible());
    }

    #[test]
    fn os_restore_while_visible_is_ignored() {
        let mut vis = VisibilityController::new(true);
        assert!(vis.on_os_restore_signal().is_empty());
    }
}
