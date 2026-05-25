use super::AgentStatus;
use gpui::{Context, EventEmitter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum AgentStatusEvent {
    Changed,
}

/// Maps each registered agent tab to its workspace and most recent status.
/// One row per tab; a workspace with no agent tabs has no entries here.
pub struct AgentStatusStore {
    entries: HashMap<String, (PathBuf, AgentStatus)>,
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
        use std::collections::hash_map::Entry;
        match self.entries.entry(tab_id) {
            Entry::Occupied(_) => {
                // Idempotent: re-registering with the same tab_id is a no-op so
                // we don't clobber a live status (e.g. Working) with Idle.
                return;
            }
            Entry::Vacant(slot) => {
                slot.insert((workspace, AgentStatus::Idle));
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

    /// Apply a hook event. Unknown `tab_id`s are dropped silently —
    /// happens when a hook fires after the tab has been closed.
    pub fn set_status(&mut self, tab_id: &str, status: AgentStatus, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get_mut(tab_id) else {
            return;
        };
        if entry.1 == status {
            return;
        }
        entry.1 = status;
        cx.emit(AgentStatusEvent::Changed);
        cx.notify();
    }

    pub fn status_for_workspace(&self, workspace: &Path) -> Option<AgentStatus> {
        aggregate_status(
            self.entries.values().map(|(p, s)| (p.as_path(), *s)),
            workspace,
        )
    }
}

impl EventEmitter<AgentStatusEvent> for AgentStatusStore {}

/// Pure aggregation helper, separated for testing. Picks the
/// highest-priority status among entries whose workspace matches.
pub fn aggregate_status<'a>(
    entries: impl IntoIterator<Item = (&'a Path, AgentStatus)>,
    workspace: &Path,
) -> Option<AgentStatus> {
    entries
        .into_iter()
        .filter(|(p, _)| *p == workspace)
        .map(|(_, s)| s)
        .max_by_key(|s| s.priority())
}
