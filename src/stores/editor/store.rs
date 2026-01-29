use super::buffer::Buffer;
use gpui::prelude::*;
use gpui::*;
use ropey::Rope;
use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum EditorStoreEvent {
    BufferOpened,
}

#[derive(Debug, Clone)]
pub enum OpenBufferError {
    NotFound,
    UnsupportedFormat(String),
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

    pub fn open_buffer(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Result<Entity<Buffer>, OpenBufferError> {
        let canonical_path = path.canonicalize().unwrap_or(path.clone());

        if let Some(buffer) = self.buffers.get(&canonical_path) {
            return Ok(buffer.clone());
        }

        if !canonical_path.exists() {
            return Err(OpenBufferError::NotFound);
        }

        // TODO: This pre-validation reads the file twice - once here and once in Buffer::load.
        // We do this because cx.new() requires a closure that returns T, not Result<T, E>.
        // Proper error handling would require rearchitecting Buffer to not need its own
        // context/entity pointer for the background polling task.
        let file = std::fs::File::open(&canonical_path)
            .map_err(|e| OpenBufferError::UnsupportedFormat(e.to_string()))?;
        Rope::from_reader(BufReader::new(file))
            .map_err(|e| OpenBufferError::UnsupportedFormat(e.to_string()))?;

        let path_for_closure = canonical_path.clone();
        let buffer = cx.new(|cx| {
            Buffer::load(path_for_closure, cx).expect("Pre-validated file should load")
        });

        self.buffers.insert(canonical_path, buffer.clone());

        cx.emit(EditorStoreEvent::BufferOpened);
        Ok(buffer)
    }
}
