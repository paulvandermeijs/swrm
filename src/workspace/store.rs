use super::{Workspace, persistence};
use gpui::{Context, EventEmitter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    Changed,
}

pub struct WorkspaceStore {
    pub workspaces: Vec<Workspace>,
}

impl WorkspaceStore {
    pub fn load(_cx: &mut Context<Self>) -> Self {
        let workspaces = persistence::load().unwrap_or_else(|err| {
            tracing::warn!(?err, "failed to load workspaces, starting empty");
            Vec::new()
        });
        Self { workspaces }
    }

    pub fn add(&mut self, ws: Workspace, cx: &mut Context<Self>) {
        if !self.workspaces.iter().any(|w| w.path == ws.path) {
            self.workspaces.push(ws);
            self.persist();
            cx.emit(WorkspaceEvent::Changed);
            cx.notify();
        }
    }

    pub fn remove(&mut self, path: &PathBuf, cx: &mut Context<Self>) {
        self.workspaces.retain(|w| &w.path != path);
        self.persist();
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }

    fn persist(&self) {
        if let Err(err) = persistence::save(&self.workspaces) {
            tracing::error!(?err, "failed to persist workspaces");
        }
    }
}

impl EventEmitter<WorkspaceEvent> for WorkspaceStore {}
