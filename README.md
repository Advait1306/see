# SEE

**SEE (Software Engineering Environment)** is a deprecated native macOS prototype for navigating, editing, running, and reviewing software without stitching together several separate applications.

SEE was built in Rust with [GPUI](https://gpui.rs). It combines a terminal-first workspace, source editor, project navigation, and Git review tools in a single native interface.

## Project status

SEE is deprecated and is no longer under active development. The core workspace, editor, terminal, file navigation, and diff-review flows are implemented, but the broader product was intended to become an agent-native software engineering environment rather than only another IDE.

I stopped developing SEE after OpenAI's [Codex](https://developers.openai.com/codex) evolved into the environment I had been trying to build: a system that can understand a codebase, carry out engineering tasks, run checks, and help review the resulting changes. SEE was developed independently and is not related to Codex; this repository remains public as a record of the technical work and the product direction it explored.

## What it does

- **Composable workspaces** — keep projects in separate workspaces, each with its own panes and file tree
- **Multi-pane terminals** — split panes horizontally or vertically and rearrange tabs with drag and drop
- **Native code editor** — rope-backed buffers, undo/redo, keyboard navigation, and Tree-sitter highlighting for 15+ languages
- **Project navigation** — browse and open project files without leaving the workspace
- **Git review** — move through changed files and inspect inline diffs by added, modified, or deleted state
- **Persistent sessions** — restore windows, layouts, sidebar state, expanded folders, and workspaces between launches

## Why this project exists

Software work often gets fragmented across a terminal multiplexer, editor, file browser, Git client, and—more recently—coding agents. SEE explored what those workflows could feel like when they shared one native state model and one interaction surface.

The implemented prototype also served as an experiment in building a substantial desktop application in Rust: terminal emulation, text editing, syntax parsing, process management, persistence, and reactive UI all live in the same codebase.

## Architecture

```text
src/
├── stores/              # Application state and events
│   ├── workspace/       # Workspaces and their persisted state
│   ├── editor/          # Rope-backed text buffers
│   ├── git/             # Repository status and diff computation
│   ├── pane_store.rs    # Split-pane tree and layout operations
│   ├── file_tree_store.rs
│   ├── terminal_store.rs
│   └── window_store.rs
├── ui/                  # GPUI views
│   ├── window_view.rs   # Root application surface
│   ├── pane/            # Pane and tab composition
│   ├── editor/          # Editor rendering and interaction
│   ├── terminal/        # Alacritty-backed terminal view
│   ├── file_tree.rs
│   ├── diff_list.rs
│   └── workspace_sidebar.rs
├── syntax/              # Tree-sitter language registry
├── commands.rs          # Actions and keybindings
├── config.rs            # JSON persistence
└── constants.rs         # Theme and layout constants
```

Stores own application state and emit events when it changes. Views read from those stores during rendering and observe them for updates, which keeps state transitions separate from presentation.

## Technical highlights

- Native UI and reactive state management with GPUI
- Terminal emulation built on `alacritty_terminal`
- Incremental syntax highlighting with Tree-sitter grammars
- Rope-based editor buffers with `ropey`
- Repository inspection and diff generation with `git2`
- Embedded application assets and a custom macOS bundle pipeline

## Build locally

### Prerequisites

- macOS
- Rust stable with edition 2024 support
- Xcode and the Xcode Command Line Tools
- Xcode's Metal Toolchain, required by the GPUI rendering stack

If the Metal Toolchain is not already installed:

```sh
xcodebuild -downloadComponent MetalToolchain
```

Build, bundle, ad-hoc sign, and launch a development app:

```sh
cargo xtask dev
```

Build a notarized release DMG:

```sh
cargo xtask release
```

The release task requires Apple developer credentials in a local `.env` file.

## Test

```sh
cargo test --lib
```

The library test suite contains more than 60 tests covering stores, buffers, syntax handling, pane operations, persistence, and rendering behavior.

## Keybindings

| Key | Action |
| --- | --- |
| `Cmd+Q` | Quit |
| `Cmd+W` | Close pane |
| `Cmd+{` / `Cmd+}` | Previous / next pane |
| `Cmd+Alt+[` / `Cmd+Alt+]` | Previous / next workspace |
| `Cmd+B` | Toggle workspace sidebar |
| `Cmd+L` | Toggle file tree |
| `Cmd+G` | Toggle diff list |
| `Cmd+P` | Open command menu |
| `J` / `K` | Previous / next diff while the diff list is focused |

## License

All rights reserved.
