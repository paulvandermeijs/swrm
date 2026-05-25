use gpui::{
    App, AppContext, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString, Styled, Subscription,
    Task, Window, div,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dock::{Panel, PanelEvent};
use gpui_component::list::{List, ListDelegate, ListItem, ListState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::notification::Notification;
use gpui_component::{ActiveTheme, IconName, IndexPath, Sizable, WindowExt, h_flex, v_flex};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use swrm::app_state::AppState;
use swrm::workspace::{self, Workspace, WorkspaceStore};

pub struct LeftSidebarPanel {
    pub state: Entity<AppState>,
    list: Entity<ListState<WorkspacesDelegate>>,
    focus_handle: FocusHandle,
    _state_sub: Subscription,
    _store_sub: Subscription,
    _status_sub: Subscription,
}

impl LeftSidebarPanel {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store: Entity<WorkspaceStore> = state.read(cx).workspaces.clone();
        let list_state = cx.new(|cx| {
            let delegate = WorkspacesDelegate::new(state.clone(), cx);
            ListState::new(delegate, window, cx).selectable(true)
        });
        let state_sub = cx.observe(&state, |this: &mut Self, _, cx| {
            this.list.update(cx, |list, cx| {
                cx.notify();
                list.delegate_mut().rebuild(cx);
            });
            cx.notify();
        });
        let store_sub = cx.observe(&store, |this: &mut Self, _, cx| {
            this.list.update(cx, |list, cx| {
                list.delegate_mut().rebuild(cx);
                cx.notify();
            });
            cx.notify();
        });
        let status_store = state.read(cx).agent_status.clone();
        let status_sub = cx.observe(&status_store, |this: &mut Self, _, cx| {
            this.list.update(cx, |list, cx| {
                cx.notify();
                list.delegate_mut().rebuild(cx);
            });
            cx.notify();
        });
        Self {
            state,
            list: list_state,
            focus_handle: cx.focus_handle(),
            _state_sub: state_sub,
            _store_sub: store_sub,
            _status_sub: status_sub,
        }
    }
}

impl Render for LeftSidebarPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .track_focus(&self.focus_handle)
            .child(
                Button::new("open-workspace")
                    .label("Open workspace\u{2026}")
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.open_workspace(window, cx);
                    })),
            )
            .child(List::new(&self.list).into_any_element())
    }
}

impl gpui::EventEmitter<PanelEvent> for LeftSidebarPanel {}

impl Focusable for LeftSidebarPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for LeftSidebarPanel {
    fn panel_name(&self) -> &'static str {
        "left-sidebar"
    }

    fn zoomable(&self, _cx: &App) -> Option<gpui_component::dock::PanelControl> {
        None
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Workspaces"
    }
}

struct Section {
    label: SharedString,
    project: PathBuf,
    workspaces: Vec<Workspace>,
}

struct WorkspacesDelegate {
    state: Entity<AppState>,
    sections: Vec<Section>,
    selected_index: Option<IndexPath>,
}

impl WorkspacesDelegate {
    fn new(state: Entity<AppState>, cx: &App) -> Self {
        let mut me = Self {
            state,
            sections: Vec::new(),
            selected_index: None,
        };
        me.rebuild(cx);
        me
    }

    fn rebuild(&mut self, cx: &App) {
        let store = self.state.read(cx).workspaces.read(cx);
        let mut by_project: BTreeMap<PathBuf, Vec<Workspace>> = BTreeMap::new();
        for ws in &store.workspaces {
            by_project
                .entry(ws.project_dir().to_path_buf())
                .or_default()
                .push(ws.clone());
        }
        self.sections = by_project
            .into_iter()
            .map(|(project, mut workspaces)| {
                workspaces.sort_by(|a, b| a.label.cmp(&b.label));
                let label = project
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| project.display().to_string())
                    .into();
                Section {
                    label,
                    project,
                    workspaces,
                }
            })
            .collect();
    }
}

impl ListDelegate for WorkspacesDelegate {
    type Item = ListItem;

    fn sections_count(&self, _cx: &App) -> usize {
        self.sections.len().max(1)
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        self.sections
            .get(section)
            .map(|s| s.workspaces.len())
            .unwrap_or(0)
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let ws = self.sections.get(ix.section)?.workspaces.get(ix.row)?;
        let active = self.state.read(cx).active_workspace.clone();
        let is_active = active.as_ref() == Some(&ws.path);
        let path = ws.path.clone();
        let state = self.state.clone();
        let is_linked_worktree = ws.path.as_path() != ws.project_dir();
        let menu_label: SharedString = if is_linked_worktree {
            "Close worktree".into()
        } else {
            "Remove from workspaces".into()
        };
        let row_id = ix.section * 10_000 + ix.row;
        let menu_state = state.clone();
        let menu_path = path.clone();
        let menu_label = menu_label.clone();
        let info = self
            .state
            .read(cx)
            .agent_status
            .read(cx)
            .workspace_info(&ws.path);
        let status = info.as_ref().map(|i| i.status);
        let message = info.and_then(|i| i.message);
        Some(
            ListItem::new(("workspace", row_id))
                .selected(is_active)
                .child(
                    v_flex()
                        .id(("workspace-content", row_id))
                        .w_full()
                        .gap_0()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(status_dot(status))
                                .child(div().flex_1().child(ws.label.clone())),
                        )
                        .child(
                            // Indent the activity line so it starts under the label text,
                            // not the dot. 8px dot + gap_2 (8px) = 16px left pad.
                            div()
                                .pl(gpui::px(16.))
                                .child(activity_line(message.as_deref(), cx)),
                        )
                        .context_menu(move |menu, _window, _cx| {
                            let state = menu_state.clone();
                            let path = menu_path.clone();
                            let label = menu_label.clone();
                            let linked = is_linked_worktree;
                            menu.item(PopupMenuItem::new(label).on_click(move |_ev, window, cx| {
                                close_workspace(&path, linked, &state, window, cx);
                            }))
                        }),
                )
                .on_click(move |_ev: &ClickEvent, _window, cx| {
                    state.update(cx, |s, cx| {
                        s.set_active_workspace(Some(path.clone()), cx);
                    });
                }),
        )
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let s = self.sections.get(section)?;
        let state = self.state.clone();
        let project = s.project.clone();
        let button_id = ("new-worktree", section);
        Some(
            h_flex()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(s.label.clone()),
                )
                .child(
                    Button::new(button_id)
                        .icon(IconName::Plus)
                        .ghost()
                        .xsmall()
                        .tooltip("New worktree")
                        .on_click(move |_ev: &ClickEvent, _window, cx| {
                            create_worktree_for(&project, &state, cx);
                        }),
                ),
        )
    }

    fn perform_search(
        &mut self,
        _query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.selected_index else {
            return;
        };
        let Some(ws) = self
            .sections
            .get(ix.section)
            .and_then(|s| s.workspaces.get(ix.row))
        else {
            return;
        };
        let path = ws.path.clone();
        self.state.update(cx, |s, cx| {
            s.set_active_workspace(Some(path), cx);
        });
    }
}

impl LeftSidebarPanel {
    fn open_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        let state = self.state.clone();
        cx.spawn_in(window, async move |_this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = cx.update(|_window, cx| add_workspace_at(path, &state, cx));
        })
        .detach();
    }
}

fn add_workspace_at<C: AppContext>(path: PathBuf, state: &Entity<AppState>, cx: &mut C) {
    let repo = match workspace::validate_repo(&path) {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!(?err, "not a git repo");
            return;
        }
    };
    let branch = workspace::current_branch(&repo);
    let project = workspace::project_dir(&repo);
    let label = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let ws = Workspace {
        label,
        path: path.clone(),
        branch,
        project: Some(project),
    };
    state.update(cx, |state, cx| {
        state.workspaces.update(cx, |store, cx| store.add(ws, cx));
        state.set_active_workspace(Some(path), cx);
    });
}

fn create_worktree_for<C: AppContext>(project: &Path, state: &Entity<AppState>, cx: &mut C) {
    let repo = match workspace::validate_repo(project) {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!(?err, "project is not a git repo");
            return;
        }
    };
    let name = workspace::random_name();
    let target = match workspace::ensure_worktree(&repo, &name) {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(?err, "ensure_worktree failed");
            return;
        }
    };
    add_workspace_at(target, state, cx);
}

fn close_workspace(
    path: &Path,
    is_linked_worktree: bool,
    state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) {
    if is_linked_worktree {
        let project = match workspace::validate_repo(path) {
            Ok(repo) => workspace::project_dir(&repo),
            Err(err) => {
                tracing::warn!(?err, "worktree is not a git repo, removing from list only");
                remove_from_store(path, state, cx);
                return;
            }
        };
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(["worktree", "remove"])
            .arg(path)
            .output();
        match output {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                tracing::warn!(stderr = %stderr, "git worktree remove failed");
                let message: SharedString = if stderr.is_empty() {
                    format!("git worktree remove exited with {}", out.status).into()
                } else {
                    stderr.into()
                };
                show_close_error(message, window, cx);
                return;
            }
            Err(err) => {
                tracing::warn!(?err, "spawn `git worktree remove` failed");
                show_close_error(format!("Failed to run git: {err}").into(), window, cx);
                return;
            }
        }
    }
    remove_from_store(path, state, cx);
}

fn show_close_error(message: SharedString, window: &mut Window, cx: &mut App) {
    window.push_notification(
        Notification::error(message)
            .title("Could not close worktree")
            .autohide(false),
        cx,
    );
}

fn remove_from_store<C: AppContext>(path: &Path, state: &Entity<AppState>, cx: &mut C) {
    let owned = path.to_path_buf();
    state.update(cx, |state, cx| {
        state
            .workspaces
            .update(cx, |store, cx| store.remove(&owned, cx));
        if state.active_workspace.as_ref() == Some(&owned) {
            state.set_active_workspace(None, cx);
        }
    });
}

fn activity_line<V>(message: Option<&str>, cx: &Context<V>) -> impl IntoElement {
    // Em dash placeholder when no activity is reported — gives users a
    // visible "nothing here right now" marker instead of an invisible
    // NBSP, and keeps the row height stable either way.
    let label = message.unwrap_or("—");
    div()
        .h(gpui::px(14.))
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .overflow_hidden()
        .child(label.to_string())
}

fn status_dot(status: Option<swrm::agent_status::AgentStatus>) -> impl IntoElement {
    use swrm::agent_status::AgentStatus;
    let base = div()
        .w(gpui::px(8.))
        .h(gpui::px(8.))
        .rounded_full()
        .flex_none();
    match status {
        Some(s) => {
            let color: u32 = match s {
                AgentStatus::Notify => 0xFFB020,
                AgentStatus::Done => 0x46A758,
                AgentStatus::Working => 0x3E63DD,
                AgentStatus::Idle => 0x7B7B7B,
            };
            base.bg(gpui::rgb(color)).into_any_element()
        }
        None => {
            // Outlined open circle — same footprint, no fill.
            base.border_1()
                .border_color(gpui::rgb(0x4A4A4A))
                .into_any_element()
        }
    }
}
