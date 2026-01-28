use super::buffer::Buffer;
use crate::syntax::LanguageRegistry;
use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
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

        // Check if file exists before attempting to load
        if !canonical_path.exists() {
            return Err(OpenBufferError::NotFound);
        }

        let path_for_closure = canonical_path.clone();
        let mut load_error: Option<String> = None;
        let buffer = cx.new(|cx| {
            match Buffer::load(path_for_closure.clone(), cx) {
                Ok(buf) => buf,
                Err(e) => {
                    eprintln!("[DEBUG] Failed to load {:?}: {}", path_for_closure, e);
                    load_error = Some(e.to_string());
                    Buffer::unsupported(path_for_closure)
                }
            }
        });

        if let Some(err) = load_error {
            return Err(OpenBufferError::UnsupportedFormat(err));
        }

        // Detect and set language based on file extension
        let registry = cx.global::<LanguageRegistry>();
        if let Some(lang) = registry.language_for_path(&canonical_path) {
            eprintln!("[DEBUG] Setting language for {:?}: {}", canonical_path, lang.name);
            buffer.update(cx, |buf, _cx| {
                buf.set_language(lang);
            });
        } else {
            eprintln!("[DEBUG] No language found for {:?}", canonical_path);
        }

        self.buffers.insert(canonical_path.clone(), buffer.clone());

        cx.emit(EditorStoreEvent::BufferOpened);
        Ok(buffer)
    }
}
