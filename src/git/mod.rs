mod diff;
mod store;

pub use diff::{compute_hunks, compute_line_diffs, LineDiff};
pub use store::{ChangedFile, FileStatus, GitStore, GitStoreEvent};
