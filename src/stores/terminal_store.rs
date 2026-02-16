use crate::terminal::Terminal;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum TerminalStoreEvent {
    TerminalCreated,
}

pub struct TerminalStore {
    terminals: HashMap<String, Entity<Terminal>>,
}

pub struct GlobalTerminalStore(pub Entity<TerminalStore>);

impl Global for GlobalTerminalStore {}

impl EventEmitter<TerminalStoreEvent> for TerminalStore {}

impl TerminalStore {
    pub fn init(cx: &mut App) {
        let store = cx.new(|_cx| Self::new());
        cx.set_global(GlobalTerminalStore(store));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalTerminalStore>().0.clone()
    }

    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
        }
    }

    pub fn create_terminal(
        &mut self,
        cwd: PathBuf,
        cx: &mut Context<Self>,
    ) -> Option<(String, Entity<Terminal>)> {
        let terminal = cx.new(|cx| Terminal::new(cwd, cx).expect("Failed to create terminal"));
        let id = Uuid::new_v4().to_string();

        self.terminals.insert(id.clone(), terminal.clone());
        cx.emit(TerminalStoreEvent::TerminalCreated);

        Some((id, terminal))
    }
}

impl Default for TerminalStore {
    fn default() -> Self {
        Self::new()
    }
}
