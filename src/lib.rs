#![recursion_limit = "1024"]

pub mod assets;
pub mod commands;
pub mod config;
pub mod constants;
pub mod file_watcher;
pub mod github;
pub mod stores;
pub mod syntax;
pub mod terminal;
pub mod types;
pub mod ui;

#[cfg(test)]
pub mod test_helpers;
