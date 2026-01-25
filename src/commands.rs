//! Application commands and keybindings
//!
//! Commands are defined as GPUI actions and bound to keystrokes.

use gpui::*;

// Pane commands (works for terminal, editor, future pane types)
actions!(
    august,
    [ClosePane, PrevPane, NextPane,]
);

// Terminal-specific commands
actions!(
    august,
    [SendTabToTerminal, SendShiftTabToTerminal,]
);

// Workspace commands
actions!(
    august,
    [PrevWorkspace, NextWorkspace,]
);

// UI commands
actions!(
    august,
    [
        ToggleWorkspaceSidebar, // Left sidebar (workspaces)
        ToggleFileTree,         // Right sidebar (file system)
        Quit,
    ]
);

/// Register all application keybindings
pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        // Application
        KeyBinding::new("cmd-q", Quit, None),
        // Pane management (terminal, editor, etc.)
        KeyBinding::new("cmd-w", ClosePane, None),
        KeyBinding::new("cmd-{", PrevPane, None),
        KeyBinding::new("cmd-}", NextPane, None),
        // Workspace switching
        KeyBinding::new("cmd-alt-[", PrevWorkspace, None),
        KeyBinding::new("cmd-alt-]", NextWorkspace, None),
        // UI toggles
        KeyBinding::new("cmd-b", ToggleWorkspaceSidebar, None), // Left sidebar
        KeyBinding::new("cmd-l", ToggleFileTree, None),         // Right sidebar
        // Tab key handling (only when Terminal has focus)
        KeyBinding::new("tab", SendTabToTerminal, Some("Terminal")),
        KeyBinding::new("shift-tab", SendShiftTabToTerminal, Some("Terminal")),
    ]);
}
