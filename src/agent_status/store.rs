use super::AgentStatus;
use gpui::{Context, EventEmitter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum AgentStatusEvent {
    Changed,
}

/// Per-workspace summary surfaced to the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceAgentInfo {
    pub status: AgentStatus,
    pub message: Option<String>,
}

/// Maps each registered agent tab to its workspace and most recent status.
/// One row per tab; a workspace with no agent tabs has no entries here.
pub struct AgentStatusStore {
    entries: HashMap<String, Entry>,
}

impl AgentStatusStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a tab so its workspace can show an indicator. Starts at
    /// `AgentStatus::Idle` — Claude's `SessionStart` hook will overwrite
    /// it within a few hundred ms, but we set a baseline now so the
    /// indicator does not flicker on between empty / first-hook.
    pub fn register(&mut self, tab_id: String, workspace: PathBuf, cx: &mut Context<Self>) {
        use std::collections::hash_map::Entry as MapEntry;
        match self.entries.entry(tab_id) {
            MapEntry::Occupied(_) => {
                // Idempotent: re-registering with the same tab_id is a no-op so
                // we don't clobber a live status (e.g. Working) with Idle.
                return;
            }
            MapEntry::Vacant(slot) => {
                slot.insert(Entry {
                    workspace,
                    status: AgentStatus::Idle,
                    message: None,
                });
            }
        }
        cx.emit(AgentStatusEvent::Changed);
        cx.notify();
    }

    pub fn unregister(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        if self.entries.remove(tab_id).is_some() {
            cx.emit(AgentStatusEvent::Changed);
            cx.notify();
        }
    }

    /// Update status; preserve the existing activity message. Use this
    /// for events that don't carry tool info (Stop, SessionStart, etc).
    pub fn set_status(&mut self, tab_id: &str, status: AgentStatus, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get_mut(tab_id) else {
            return;
        };
        if entry.status == status {
            return;
        }
        entry.status = status;
        cx.emit(AgentStatusEvent::Changed);
        cx.notify();
    }

    /// Update status AND replace the message. Use this for events that
    /// carry tool info (PreToolUse/PostToolUse/PermissionRequest).
    pub fn set_status_with_message(
        &mut self,
        tab_id: &str,
        status: AgentStatus,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.entries.get_mut(tab_id) else {
            return;
        };
        if entry.status == status && entry.message == message {
            return;
        }
        entry.status = status;
        entry.message = message;
        cx.emit(AgentStatusEvent::Changed);
        cx.notify();
    }

    pub fn workspace_info(&self, workspace: &Path) -> Option<WorkspaceAgentInfo> {
        aggregate_workspace_info(
            self.entries
                .values()
                .map(|e| (e.workspace.as_path(), e.status, e.message.as_deref())),
            workspace,
        )
    }
}

impl EventEmitter<AgentStatusEvent> for AgentStatusStore {}

/// Pure aggregation helper, separated for testing. Picks the
/// highest-priority status among entries whose workspace matches,
/// and returns the message associated with that entry.
pub fn aggregate_workspace_info<'a>(
    entries: impl IntoIterator<Item = (&'a Path, AgentStatus, Option<&'a str>)>,
    workspace: &Path,
) -> Option<WorkspaceAgentInfo> {
    entries
        .into_iter()
        .filter(|(p, _, _)| *p == workspace)
        .max_by_key(|(_, s, _)| s.priority())
        .map(|(_, status, message)| WorkspaceAgentInfo {
            status,
            message: message.map(|s| s.to_string()),
        })
}

struct Entry {
    workspace: PathBuf,
    status: AgentStatus,
    message: Option<String>,
}
