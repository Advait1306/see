use crate::types::Tab;
use crate::types::TabConfig;
use crate::ui::{EditorView, TerminalView};
use gpui::*;

use super::Pane;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Up,
    Down,
    Left,
    Right,
}

impl SplitDirection {
    pub fn axis(&self) -> Axis {
        match self {
            SplitDirection::Up | SplitDirection::Down => Axis::Vertical,
            SplitDirection::Left | SplitDirection::Right => Axis::Horizontal,
        }
    }

    pub fn is_before(&self) -> bool {
        matches!(self, SplitDirection::Up | SplitDirection::Left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Represents a tab item which can be either a terminal or an editor
#[derive(Clone)]
pub enum TabItem {
    Terminal(Entity<TerminalView>),
    Editor(Entity<EditorView>),
}

impl TabItem {
    pub fn label(&self, cx: &App) -> String {
        match self {
            TabItem::Terminal(t) => t.read(cx).label(cx),
            TabItem::Editor(e) => e.read(cx).label(cx),
        }
    }

    pub fn to_config(&self, cx: &App) -> TabConfig {
        match self {
            TabItem::Terminal(t) => t.read(cx).to_config(cx),
            TabItem::Editor(e) => e.read(cx).to_config(cx),
        }
    }
}

pub enum PaneEvent {
    Split {
        direction: SplitDirection,
        new_pane: Entity<Pane>,
    },
    TabMoved,
    TerminalAdded,
    TabClosed,
    Focus,
}
