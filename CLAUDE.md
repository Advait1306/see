# Claude Instructions

- Never run `cargo run` - always let the user run it themselves
- Always bundle the app (`cargo bundle --release`) after building for final review by the user
- Always use `cargo add <package>` to add dependencies (without version numbers - let cargo get the latest), never manually edit Cargo.toml
