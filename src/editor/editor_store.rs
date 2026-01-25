use super::buffer::{Buffer, BufferEvent};
use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

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
        let store = cx.new(|cx| Self::new(cx));
        cx.set_global(GlobalEditorStore(store));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalEditorStore>().0.clone()
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;

                let _ = this.update(cx, |store, cx| {
                    store.check_all_external_changes(cx);
                });
            }
        })
        .detach();

        Self {
            buffers: HashMap::new(),
        }
    }

    pub fn open_buffer(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Option<Entity<Buffer>> {
        let canonical_path = path.canonicalize().unwrap_or(path.clone());

        if let Some(buffer) = self.buffers.get(&canonical_path) {
            return Some(buffer.clone());
        }

        let buffer = cx.new(|cx| {
            Buffer::load(canonical_path.clone(), cx).unwrap_or_else(|_| {
                panic!("Failed to load buffer for {:?}", canonical_path)
            })
        });

        self.buffers.insert(canonical_path.clone(), buffer.clone());

        cx.emit(EditorStoreEvent::BufferOpened);
        Some(buffer)
    }

    fn check_all_external_changes(&mut self, cx: &mut Context<Self>) {
        for (_path, buffer) in &self.buffers {
            let has_changes = buffer.read(cx).check_external_changes();
            let is_dirty = buffer.read(cx).is_dirty();

            if has_changes {
                buffer.update(cx, |buf, cx| {
                    cx.emit(BufferEvent::ExternalChange);
                    if !is_dirty {
                        let _ = buf.reload(cx);
                    }
                });
            }
        }
    }
}
