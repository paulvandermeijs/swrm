use crate::workspace::WorkspaceStore;
use gpui::{AppContext, Context, Entity, EventEmitter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    ActiveWorkspaceChanged,
    SidebarToggled,
}

pub struct AppState {
    pub workspaces: Entity<WorkspaceStore>,
    pub active_workspace: Option<PathBuf>,
    pub left_sidebar_visible: bool,
    pub right_sidebar_visible: bool,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let workspaces = cx.new(|cx| WorkspaceStore::load(cx));
        Self {
            workspaces,
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
