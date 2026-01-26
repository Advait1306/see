# State Architecture

This document describes the state management patterns used in this application, based on GPUI's reactive model.

## Core Concepts

### Entity<T>

An `Entity<T>` is a handle to data owned by GPUI's `App`. It's similar to `Arc<T>` - cloning it is cheap (just increments a reference count). The actual data lives in one place, owned by the application.

```rust
let store: Entity<MyStore> = cx.new(|cx| MyStore::new());

// Cloning is cheap - no data copying
let store_clone = store.clone();

// Read data (immutable access)
let value = store.read(cx).some_field;

// Update data (mutable access)
store.update(cx, |store, cx| {
    store.some_field = new_value;
    cx.notify();  // Notify observers
});
```

### cx.notify()

Calling `cx.notify()` tells GPUI that an entity has changed. All observers of that entity will be notified, triggering their callbacks.

### cx.observe()

Views use `cx.observe()` to watch for changes in stores. When the observed entity calls `cx.notify()`, the callback fires.

```rust
cx.observe(&store, |this, store, cx| {
    // Called whenever store calls cx.notify()
    cx.notify();  // Re-render this view
}).detach();
```

### Global Stores

For app-wide stores, use GPUI's `Global` trait:

```rust
pub struct GlobalWorkspaceStore(pub Entity<WorkspaceStore>);
impl Global for GlobalWorkspaceStore {}

// Set during initialization
cx.set_global(GlobalWorkspaceStore(store));

// Access from anywhere
let store = cx.global::<GlobalWorkspaceStore>().0.clone();
```

---

## Simple Stores

For standalone stores without parent-child relationships, use the **observe + notify** pattern.

### The Pattern

1. **Store holds state, calls `cx.notify()` on changes**
2. **View observes store, reads data in `render()`**
3. **No data copying** - view reads directly from store

### 1. Store Holds State, Calls notify() on Changes

```rust
pub struct MyStore {
    data: Vec<Item>,
    selected_index: Option<usize>,
}

impl MyStore {
    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_index = Some(index);
        cx.notify();  // Notify all observers
    }

    pub fn add_item(&mut self, item: Item, cx: &mut Context<Self>) {
        self.data.push(item);
        cx.notify();  // Notify all observers
    }
}
```

### 2. View Observes Store, Reads Data in render()

```rust
pub struct MyView {
    store: Entity<MyStore>,
}

impl MyView {
    pub fn new(store: Entity<MyStore>, cx: &mut Context<Self>) -> Self {
        // Observe the store - re-render when it changes
        cx.observe(&store, |_, _, cx| cx.notify()).detach();

        Self { store }
    }
}

impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Read directly from store - no local copy needed
        let items = &self.store.read(cx).data;
        let selected = self.store.read(cx).selected_index;

        div().children(items.iter().enumerate().map(|(i, item)| {
            let is_selected = selected == Some(i);
            self.render_item(item, is_selected)
        }))
    }
}
```

### 3. Data Flow

```
User Action
    │
    ▼
Store Method Called
    │
    ├── Mutate state
    │
    └── cx.notify()
           │
           ▼
    GPUI notifies all observers
           │
           ▼
    View's observe callback fires
           │
           └── cx.notify() on view
                  │
                  ▼
           GPUI calls render()
                  │
                  └── View reads fresh data from store
                         │
                         ▼
                  UI updates automatically
```

### No Events, No Data Copying

This pattern avoids two common anti-patterns:

| Anti-Pattern | Problem | Solution |
|--------------|---------|----------|
| Custom events between store and view | Boilerplate, manual wiring | Use `cx.observe()` + `cx.notify()` |
| Copying data from store to view | Duplicate state, sync issues | Read from store in `render()` |

The view holds an `Entity<Store>` handle, not a copy of the data. During render, it reads directly from the store. When the store changes, the view re-renders and reads the new data.

---

## Example: Zed's AutoUpdater

Zed's auto-update system demonstrates the simple store pattern.

### Store Definition

```rust
// From zed/crates/auto_update/src/auto_update.rs

pub struct AutoUpdater {
    status: AutoUpdateStatus,
    current_version: Version,
    // ...
}

pub enum AutoUpdateStatus {
    Idle,
    Checking,
    Downloading { version: VersionCheckType },
    Installing { version: VersionCheckType },
    Updated { version: VersionCheckType },
    Errored { error: Arc<anyhow::Error> },
}
```

### Store Mutations with notify()

```rust
async fn update(this: Entity<Self>, cx: &mut AsyncApp) -> Result<()> {
    // Stage 1: Checking
    this.update(cx, |this, cx| {
        this.status = AutoUpdateStatus::Checking;
        cx.notify();
    });

    // ... fetch release ...

    // Stage 2: Downloading
    this.update(cx, |this, cx| {
        this.status = AutoUpdateStatus::Downloading { version };
        cx.notify();
    });

    // ... download ...

    // Stage 3: Installing
    this.update(cx, |this, cx| {
        this.status = AutoUpdateStatus::Installing { version };
        cx.notify();
    });

    // ... install ...

    // Stage 4: Done
    this.update(cx, |this, cx| {
        this.status = AutoUpdateStatus::Updated { version };
        cx.notify();
    });
}
```

### View Observes Store

```rust
// From zed/crates/activity_indicator/src/activity_indicator.rs

impl ActivityIndicator {
    pub fn new(/* ... */) -> Entity<ActivityIndicator> {
        let auto_updater = AutoUpdater::get(cx);

        cx.new(|cx| {
            // Observe the auto_updater - re-render when it changes
            if let Some(auto_updater) = auto_updater.as_ref() {
                cx.observe(auto_updater, |_, _, cx| cx.notify()).detach();
            }

            Self {
                auto_updater,
                // ...
            }
        })
    }
}
```

### View Reads Store in Render

```rust
impl ActivityIndicator {
    fn content(&self, cx: &App) -> Option<Content> {
        // Read directly from store - no local copy
        self.auto_updater
            .as_ref()
            .and_then(|updater| match &updater.read(cx).status() {
                AutoUpdateStatus::Checking => Some(Content {
                    message: "Checking for Zed updates...".to_string(),
                    // ...
                }),
                AutoUpdateStatus::Downloading { .. } => Some(Content {
                    message: "Downloading Zed update...".to_string(),
                    // ...
                }),
                AutoUpdateStatus::Installing { .. } => Some(Content {
                    message: "Installing Zed update...".to_string(),
                    // ...
                }),
                AutoUpdateStatus::Updated { .. } => Some(Content {
                    message: "Click to restart and update Zed".to_string(),
                    // ...
                }),
                AutoUpdateStatus::Errored { .. } => Some(Content {
                    message: "Failed to update Zed".to_string(),
                    // ...
                }),
                AutoUpdateStatus::Idle => None,
            })
    }
}
```

### Complete Data Flow

```
AutoUpdater                          ActivityIndicator
───────────                          ─────────────────
     │                                      │
     │  status = Checking                   │
     │  cx.notify() ──────────────────────► │ observe callback fires
     │                                      │ cx.notify() on self
     │                                      │ render() called
     │                                      │ updater.read(cx).status()
     │                                      │ shows "Checking..."
     │                                      │
     │  status = Downloading                │
     │  cx.notify() ──────────────────────► │ observe callback fires
     │                                      │ shows "Downloading..."
     │                                      │
     │  status = Installing                 │
     │  cx.notify() ──────────────────────► │ observe callback fires
     │                                      │ shows "Installing..."
     │                                      │
     │  status = Updated                    │
     │  cx.notify() ──────────────────────► │ observe callback fires
     │                                      │ shows "Click to restart"
```

---

## Nested Stores

When a parent entity contains child stores, use **event bubbling** - the parent subscribes to child stores and re-emits their events as its own.

### The Pattern

1. **Parent contains child stores as `Entity<T>` fields**
2. **Parent subscribes to each child store during initialization**
3. **Parent re-emits child events as parent events**
4. **Views subscribe to parent, not child stores directly**

### 1. Parent Contains Child Stores

```rust
pub struct Parent {
    child_a: Entity<ChildStoreA>,
    child_b: Entity<ChildStoreB>,
    _subscriptions: Vec<Subscription>,  // Keep subscriptions alive
}
```

### 2. Define Parent Events

The parent defines its own event type that aggregates child events:

```rust
pub enum ParentEvent {
    ChildAChanged,
    ChildBChanged,
    ChildBItemAdded { id: String },
}

impl EventEmitter<ParentEvent> for Parent {}
```

### 3. Parent Subscribes to Children and Re-emits

During initialization, the parent subscribes to each child and converts their events:

```rust
impl Parent {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let child_a = cx.new(|_| ChildStoreA::new());
        let child_b = cx.new(|_| ChildStoreB::new());

        // Subscribe to child_a, re-emit as parent event
        let sub_a = cx.subscribe(&child_a, |_this, _store, event, cx| {
            cx.emit(ParentEvent::ChildAChanged);
        });

        // Subscribe to child_b, convert events
        let sub_b = cx.subscribe(&child_b, |_this, _store, event, cx| {
            match event {
                ChildBEvent::ItemAdded { id } => {
                    cx.emit(ParentEvent::ChildBItemAdded { id: id.clone() });
                }
                _ => {
                    cx.emit(ParentEvent::ChildBChanged);
                }
            }
        });

        Self {
            child_a,
            child_b,
            _subscriptions: vec![sub_a, sub_b],
        }
    }
}
```

### 4. Views Subscribe to Parent

Views subscribe to the parent entity, not individual child stores:

```rust
impl MyView {
    pub fn new(parent: Entity<Parent>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&parent, |this, parent, event, cx| {
            match event {
                ParentEvent::ChildAChanged => {
                    // React to child_a changes
                    cx.notify();
                }
                ParentEvent::ChildBItemAdded { id } => {
                    // React to specific child_b event
                    log::info!("Item added: {}", id);
                    cx.notify();
                }
                _ => {}
            }
        }).detach();

        Self { parent }
    }
}
```

### Data Flow

```
ChildStoreA                      Parent                         View
───────────                      ──────                         ────
     │                              │                              │
     │ ChildAEvent ────────────────►│                              │
     │                              │ on_child_a_event()           │
     │                              │ cx.emit(ParentEvent::        │
     │                              │         ChildAChanged)       │
     │                              │ ────────────────────────────►│
     │                              │                              │ match event
     │                              │                              │ cx.notify()
     │                              │                              │ render()
```

### Benefits

- **Unified API**: Views see one event type, not multiple child event types
- **Decoupling**: Views don't need to know about child store implementation details
- **Coordination**: Parent can implement logic that spans multiple child stores
- **Filtering**: Views can react to specific events, not just "something changed"

### When to Access Child Stores Directly

Sometimes views need to read data from child stores. Access them through the parent:

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let parent = self.parent.read(cx);
        let child_a_data = parent.child_a.read(cx).get_data();
        // ...
    }
}
```

Or expose accessor methods on the parent:

```rust
impl Parent {
    pub fn child_a(&self) -> &Entity<ChildStoreA> {
        &self.child_a
    }
}
```

---

## Example: Zed's Project and WorktreeStore

Zed's `Project` entity contains multiple child stores (`WorktreeStore`, `BufferStore`, `GitStore`, etc.) and demonstrates the nested stores pattern.

### Parent Contains Child Stores

```rust
// From zed/crates/project/src/project.rs

pub struct Project {
    worktree_store: Entity<WorktreeStore>,
    buffer_store: Entity<BufferStore>,
    git_store: Entity<GitStore>,
    lsp_store: Entity<LspStore>,
    // ...
    _subscriptions: Vec<gpui::Subscription>,
}
```

### Parent Subscribes to Children

```rust
// From project.rs - during initialization

let worktree_store = cx.new(|_| WorktreeStore::local(false, fs.clone()));
cx.subscribe(&worktree_store, Self::on_worktree_store_event).detach();

let buffer_store = cx.new(|cx| BufferStore::local(worktree_store.clone(), cx));
cx.subscribe(&buffer_store, Self::on_buffer_store_event).detach();

let lsp_store = cx.new(|cx| LspStore::new_local(...));
cx.subscribe(&lsp_store, Self::on_lsp_store_event).detach();
```

### Parent Re-emits Child Events

```rust
// From project.rs

fn on_worktree_store_event(
    &mut self,
    _: Entity<WorktreeStore>,
    event: &WorktreeStoreEvent,
    cx: &mut Context<Self>,
) {
    match event {
        WorktreeStoreEvent::WorktreeAdded(worktree) => {
            self.on_worktree_added(worktree, cx);
            // Re-emit as Project event
            cx.emit(Event::WorktreeAdded(worktree.read(cx).id()));
        }
        WorktreeStoreEvent::WorktreeRemoved(_, id) => {
            cx.emit(Event::WorktreeRemoved(*id));
        }
        WorktreeStoreEvent::WorktreeOrderChanged => {
            cx.emit(Event::WorktreeOrderChanged);
        }
        WorktreeStoreEvent::WorktreeUpdatedEntries(worktree, changes) => {
            cx.emit(Event::WorktreeUpdatedEntries(
                worktree.read(cx).id(),
                changes.clone(),
            ));
        }
        // ...
    }
}
```

### View Subscribes to Parent

```rust
// From zed/crates/project_panel/src/project_panel.rs

cx.subscribe_in(
    &project,  // Subscribe to parent Project
    window,
    |this, project, event, window, cx| match event {
        project::Event::WorktreeAdded(_) => {
            this.update_visible_entries(None, false, false, window, cx);
            cx.notify();
        }
        project::Event::WorktreeUpdatedEntries(_, _) => {
            this.update_visible_entries(None, false, false, window, cx);
            cx.notify();
        }
        project::Event::WorktreeRemoved(_) => {
            this.update_visible_entries(None, false, false, window, cx);
            cx.notify();
        }
        // ...
    },
)
.detach();
```

### Complete Data Flow

```
WorktreeStore                    Project                      ProjectPanel
─────────────                    ───────                      ────────────
     │                              │                              │
     │ WorktreeStoreEvent::         │                              │
     │ WorktreeAdded ──────────────►│                              │
     │                              │ on_worktree_store_event()    │
     │                              │ cx.emit(Event::WorktreeAdded)│
     │                              │ ────────────────────────────►│
     │                              │                              │ update_visible_entries()
     │                              │                              │ cx.notify()
     │                              │                              │ render()
```

---

## When to Use Events (cx.emit / cx.subscribe)

The `observe` + `notify` pattern covers most cases. Use typed events (`cx.emit` + `cx.subscribe`) when:

1. **Different observers need different behavior** - e.g., one view refreshes, another logs
2. **Events carry data** - e.g., `FileDeleted { path }` vs just "something changed"
3. **Filtering is needed** - e.g., only react to certain event types
4. **Nested stores** - parent needs to re-emit child events

```rust
// Define event type
pub enum StoreEvent {
    ItemAdded { id: String },
    ItemRemoved { id: String },
}

// Store emits events
impl EventEmitter<StoreEvent> for MyStore {}

impl MyStore {
    pub fn remove(&mut self, id: &str, cx: &mut Context<Self>) {
        self.items.remove(id);
        cx.emit(StoreEvent::ItemRemoved { id: id.to_string() });
        cx.notify();
    }
}

// View subscribes to specific events
cx.subscribe(&store, |this, _store, event, cx| {
    match event {
        StoreEvent::ItemRemoved { id } => {
            log::info!("Item removed: {}", id);
        }
        _ => {}
    }
    cx.notify();
}).detach();
```

For simple "state changed, re-render" needs, prefer `observe` + `notify`.

---

## Summary

| Pattern | Use Case | Mechanism |
|---------|----------|-----------|
| Simple Store | Standalone state | `cx.observe()` + `cx.notify()` |
| Nested Stores | Parent with child stores | `cx.subscribe()` + `cx.emit()` with event bubbling |

| Component | Responsibility |
|-----------|----------------|
| Store | Hold state, call `cx.notify()` on changes |
| View | Observe/subscribe to store, read data in `render()` |
| Entity<T> | Handle to store (cheap to clone) |
| cx.notify() | Signal that state changed |
| cx.observe() | React to any state change |
| cx.emit() | Emit typed event |
| cx.subscribe() | React to specific event types |

The pattern is simple: **stores notify, views observe and read**.
