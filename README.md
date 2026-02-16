# August

A native macOS IDE built with Rust and [GPUI](https://gpui.rs), featuring integrated terminals, a code editor with syntax highlighting, file tree navigation, and git diff viewing.

## Features

- **Multi-pane terminals** — Split panes horizontally and vertically with drag-and-drop tab rearrangement, powered by Alacritty's terminal emulator
- **Code editor** — Syntax highlighting via Tree-sitter for 15+ languages (Rust, TypeScript, Python, Go, C/C++, and more), with undo/redo and keyboard navigation
- **File tree** — Browse and open files from the right sidebar
- **Git diff viewer** — Carousel through changed files with inline diffs, showing added/modified/deleted status
- **Workspaces** — Organize projects into separate workspaces, each with its own pane layout and file tree
- **Persistent state** — Window layout, sidebar visibility, expanded folders, and workspace list are saved across sessions

## Keybindings

| Key | Action |
|-----|--------|
| `Cmd+Q` | Quit |
| `Cmd+W` | Close pane |
| `Cmd+{` / `Cmd+}` | Previous / next pane |
| `Cmd+Alt+[` / `Cmd+Alt+]` | Previous / next workspace |
| `Cmd+B` | Toggle workspace sidebar (left) |
| `Cmd+L` | Toggle file tree (right) |
| `Cmd+G` | Toggle diff list (right) |
| `J` / `K` | Previous / next diff (when diff list is focused) |

## Building

Requires Rust (edition 2024) and macOS.

**Development build:**

```sh
cargo xtask dev
```

This builds a debug bundle, ad-hoc signs it, and launches the app.

**Release build:**

```sh
cargo xtask release
```

Builds an optimized, code-signed, and notarized `.dmg` (requires Apple developer credentials in `.env`).

## Testing

```sh
cargo test --lib
```

Runs 70 tests covering stores, buffers, syntax highlighting, pane management, and UI rendering.

## Architecture

```
src/
├── stores/          # State management (GPUI Global pattern)
│   ├── workspace/   # Workspace & WorkspaceStore
│   ├── editor/      # EditorStore, Buffer (rope-based)
│   ├── git/         # GitStore, diff computation
│   ├── pane_store   # Pane tree layout
│   ├── file_tree_store
│   ├── terminal_store
│   └── window_store # Per-window UI state
├── ui/              # View components (each in its own file)
│   ├── window_view  # Root container
│   ├── pane/        # Pane with tab bar
│   ├── editor/      # Code editor view
│   ├── terminal/    # Terminal view
│   ├── file_tree    # File navigator
│   ├── diff_list    # Diff carousel
│   └── workspace_sidebar
├── syntax/          # Tree-sitter language registry
├── commands.rs      # Actions & keybindings
├── config.rs        # JSON persistence (~/.local/share/August-Dev/)
└── constants.rs     # Colors (Catppuccin Mocha), dimensions
```

**Stores** hold all application state and emit events on changes. **Views** read from stores during render and observe them for re-renders. Views never hold copies of store state.

## License

All rights reserved.
