use super::buffer::Buffer;
use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum EditorStoreEvent {
    BufferOpened,
}

pub struct EditorStore {
    buffers: HashMap<PathBuf, Entity<Buffer>>,
}

pub struct GlobalEditorStore(pub Entity<EditorStore>);

impl Global for GlobalEditorStore {}

impl EventEmitter<EditorStoreEvent> for EditorStore {}

impl EditorStore {
    pub fn init(cx: &mut App) {
        let store = cx.new(|_cx| Self::new());
        cx.set_global(GlobalEditorStore(store));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalEditorStore>().0.clone()
    }

    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    pub fn open_buffer(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Option<Entity<Buffer>> {
        let canonical_path = path.canonicalize().unwrap_or(path.clone());

        if let Some(buffer) = self.buffers.get(&canonical_path) {
            return Some(buffer.clone());
        }

        // Check if file exists before attempting to load
        if !canonical_path.exists() {
            return None;
        }

        let buffer = cx.new(|cx| {
            Buffer::load(canonical_path.clone(), cx)
                .expect("File existed but failed to load")
        });

        self.buffers.insert(canonical_path.clone(), buffer.clone());

        cx.emit(EditorStoreEvent::BufferOpened);
        Some(buffer)
    }
}
