use crate::agent_status::{AgentStatus, AgentStatusStore, start_server};
use crate::settings::SettingsStore;
use crate::workspace::WorkspaceStore;
use futures::StreamExt;
use gpui::{AppContext, Context, Entity, EventEmitter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    ActiveWorkspaceChanged,
    SidebarToggled,
}

pub struct AppState {
    pub workspaces: Entity<WorkspaceStore>,
    pub settings: Entity<SettingsStore>,
    pub agent_status: Entity<AgentStatusStore>,
    /// Origin of the agent-status hook server (`http://127.0.0.1:<port>`).
    /// Baked into each tab's generated Claude settings file.
    pub agent_status_origin: String,
    pub active_workspace: Option<PathBuf>,
    pub left_sidebar_visible: bool,
    pub right_sidebar_visible: bool,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let workspaces = cx.new(|cx| WorkspaceStore::load(cx));
        let settings = cx.new(|cx| SettingsStore::load(cx));
        let agent_status = cx.new(|_cx| AgentStatusStore::new());

        let (port, mut rx) = match start_server() {
            Ok(pair) => pair,
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "agent-status server failed to start; tabs will spawn without status tracking"
                );
                // Sentinel that won't match a real loopback — guarantees the
                // placeholder substitution still produces a file (so the
                // user's `claude --settings $CLAUDE_SETTINGS` invocation
                // doesn't break), it just won't deliver events anywhere.
                (0u16, futures::channel::mpsc::unbounded().1)
            }
        };
        let agent_status_origin = format!("http://127.0.0.1:{port}");

        let store_weak = agent_status.downgrade();
        cx.spawn(async move |_, cx| {
            while let Some(evt) = rx.next().await {
                let Some(status) = AgentStatus::from_wire(&evt.event) else {
                    tracing::debug!(event = %evt.event, "ignoring unknown status");
                    continue;
                };
                let _ = store_weak.update(cx, |store, cx| {
                    store.set_status(&evt.tab_id, status, cx);
                });
            }
        })
        .detach();

        Self {
            workspaces,
            settings,
            agent_status,
            agent_status_origin,
            active_workspace: None,
            left_sidebar_visible: true,
            right_sidebar_visible: true,
        }
    }

    pub fn set_active_workspace(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.active_workspace != path {
            self.active_workspace = path;
            cx.emit(AppEvent::ActiveWorkspaceChanged);
            cx.notify();
        }
    }

    pub fn toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.left_sidebar_visible = !self.left_sidebar_visible;
        cx.emit(AppEvent::SidebarToggled);
        cx.notify();
    }

    pub fn toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.right_sidebar_visible = !self.right_sidebar_visible;
        cx.emit(AppEvent::SidebarToggled);
        cx.notify();
    }
}

impl EventEmitter<AppEvent> for AppState {}
