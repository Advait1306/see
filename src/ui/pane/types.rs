use crate::types::Tab;
use crate::types::TabConfig;
use crate::ui::pr_review::PrReviewView;
use crate::ui::{EditorView, TerminalView};
use gpui::*;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Represents a tab item which can be a terminal, editor, or PR review
#[derive(Clone)]
pub enum TabItem {
    Terminal(Entity<TerminalView>),
    Editor(Entity<EditorView>),
    PrReview(Entity<PrReviewView>),
}

impl TabItem {
    pub fn label(&self, cx: &App) -> String {
        match self {
            TabItem::Terminal(t) => t.read(cx).label(cx),
            TabItem::Editor(e) => e.read(cx).label(cx),
            TabItem::PrReview(p) => p.read(cx).label(cx),
        }
    }

    pub fn to_config(&self, cx: &App) -> TabConfig {
        match self {
            TabItem::Terminal(t) => t.read(cx).to_config(cx),
            TabItem::Editor(e) => e.read(cx).to_config(cx),
            TabItem::PrReview(p) => p.read(cx).to_config(cx),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &App) {
        match self {
            TabItem::Terminal(t) => t.read(cx).focus_handle(cx).focus(window),
            TabItem::Editor(e) => e.read(cx).focus_handle(cx).focus(window),
            TabItem::PrReview(p) => p.read(cx).focus_handle(cx).focus(window),
        }
    }
}

pub enum PaneEvent {
    Split {
        direction: SplitDirection,
        new_pane: Entity<Pane>,
    },
    TabMoved,
    TabAdded,
    TabClosed,
    Focus,
}
