# Claude Instructions

- Never commit or push code unless the user explicitly asks you to
- Reference codebase: Zed editor is available at ~/Desktop/projects/zed for architecture reference (don't search the web for it)
- Never run `cargo run` - always let the user run it themselves
- Always bundle the app (`cargo bundle --release`) after building for final review by the user
- Always use `cargo add <package>` to add dependencies (without version numbers - let cargo get the latest), never manually edit Cargo.toml

## Code Style

- Only add comments on code that's hard to read at first glance or to document behavior that isn't obvious
- Avoid comments that merely restate what the code does (e.g., `// Get the config directory for the app` above a function named `config_dir()`)
- Only use `.map()` for arrays/iterators, not for `Option` - use `if let` or `let ... else` instead

## Architecture

- All views (types that implement `Render`) should have their own file
- Stores should not contain views - keep rendering logic in the `ui/` directory

## Migration Strategy

When making changes to persisted state formats:

1. **Each store is responsible for its own migration** - stores should detect old formats and migrate on initialization
2. **Read old data, write new format** - when a store initializes, it should:
   - Check if old format exists (e.g., legacy `state.json`)
   - Read and parse the relevant section
   - Convert to new format and save to new location
3. **Coordinate deletion** - old state files should only be deleted after all stores have successfully migrated
4. **Version markers** - consider adding version fields to JSON files to detect format changes
5. **Graceful fallbacks** - if migration fails, use defaults rather than crashing
