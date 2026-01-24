use super::buffer::{Buffer, BufferEvent};
use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum BufferStoreEvent {
    BufferOpened,
}

struct BufferEntry {
    buffer: Entity<Buffer>,
    ref_count: usize,
}

pub struct BufferStore {
    buffers: HashMap<PathBuf, BufferEntry>,
}

impl EventEmitter<BufferStoreEvent> for BufferStore {}

impl BufferStore {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Set up polling for external changes every 500ms
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
        // Canonicalize path for consistent lookup
        let canonical_path = path.canonicalize().unwrap_or(path.clone());

        if let Some(entry) = self.buffers.get_mut(&canonical_path) {
            entry.ref_count += 1;
            return Some(entry.buffer.clone());
        }

        // Create new buffer
        let buffer = cx.new(|cx| {
            Buffer::load(canonical_path.clone(), cx).unwrap_or_else(|_| {
                // If file doesn't exist or can't be read, create empty buffer
                // This shouldn't happen normally since we're opening from file tree
                panic!("Failed to load buffer for {:?}", canonical_path)
            })
        });

        self.buffers.insert(
            canonical_path.clone(),
            BufferEntry {
                buffer: buffer.clone(),
                ref_count: 1,
            },
        );

        cx.emit(BufferStoreEvent::BufferOpened);
        Some(buffer)
    }

    fn check_all_external_changes(&mut self, cx: &mut Context<Self>) {
        for (_path, entry) in &self.buffers {
            let buffer = entry.buffer.clone();
            let has_changes = buffer.read(cx).check_external_changes();
            let is_dirty = buffer.read(cx).is_dirty();

            if has_changes {
                buffer.update(cx, |buf, cx| {
                    cx.emit(BufferEvent::ExternalChange);
                    // Auto-reload if not dirty
                    if !is_dirty {
                        let _ = buf.reload(cx);
                    }
                });
            }
        }
    }
}
