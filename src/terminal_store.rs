use crate::terminal::Terminal;
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TerminalStoreEvent {
    TerminalCreated(String),
    TerminalClosed(String),
}

pub struct TerminalStore {
    terminals: HashMap<String, Arc<parking_lot::Mutex<Terminal>>>,
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
    ) -> Option<(String, Arc<parking_lot::Mutex<Terminal>>)> {
        let terminal = Terminal::new(cwd).ok()?;
        let terminal = Arc::new(parking_lot::Mutex::new(terminal));
        let id = Uuid::new_v4().to_string();

        self.terminals.insert(id.clone(), terminal.clone());
        cx.emit(TerminalStoreEvent::TerminalCreated(id.clone()));

        Some((id, terminal))
    }

    #[allow(dead_code)]
    pub fn get_terminal(&self, id: &str) -> Option<Arc<parking_lot::Mutex<Terminal>>> {
        self.terminals.get(id).cloned()
    }

    #[allow(dead_code)]
    pub fn remove_terminal(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if self.terminals.remove(id).is_some() {
            cx.emit(TerminalStoreEvent::TerminalClosed(id.to_string()));
            true
        } else {
            false
        }
    }
}

impl Default for TerminalStore {
    fn default() -> Self {
        Self::new()
    }
}
