//! Shared selection types for editor and terminal views

/// Represents the current phase of text selection
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectionPhase {
    /// No selection is active
    None,
    /// User is actively selecting (mouse button held)
    Selecting,
    /// Selection is complete (mouse released)
    Ended,
}
