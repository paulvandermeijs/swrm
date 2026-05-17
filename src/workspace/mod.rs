pub mod persistence;
pub mod store;
pub mod worktree;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub label: String,
    pub path: PathBuf,
    pub branch: Option<String>,
}
