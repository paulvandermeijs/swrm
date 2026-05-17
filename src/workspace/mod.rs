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
    /// Workdir of the workspace's project (the main repo this worktree belongs
    /// to). Workspaces that share a `project` are grouped together in the UI.
    /// Optional for back-compat with config files written before this field;
    /// fall back to `path` when reading.
    #[serde(default)]
    pub project: Option<PathBuf>,
}

impl Workspace {
    /// Returns the project directory this workspace belongs to. Falls back to
    /// `path` if `project` was not recorded (e.g. older persisted entries).
    pub fn project_dir(&self) -> &std::path::Path {
        self.project.as_deref().unwrap_or(&self.path)
    }
}

pub use store::{WorkspaceEvent, WorkspaceStore};
pub use worktree::{
    current_branch, ensure_worktree, list_worktrees, project_dir, random_name, validate_repo,
};
