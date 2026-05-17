use gpui::{
    App, AppContext, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString, Styled, Subscription,
    Task, Window, div,
};
use gpui_component::button::Button;
use gpui_component::dock::{Panel, PanelEvent};
use gpui_component::list::{List, ListDelegate, ListItem, ListState};
use gpui_component::{ActiveTheme, IndexPath, h_flex, v_flex};
use std::collections::BTreeMap;
use std::path::PathBuf;
use swrm::app_state::AppState;
use swrm::workspace::{self, Workspace, WorkspaceStore};

struct Section {
    label: SharedString,
    workspaces: Vec<Workspace>,
}

pub struct WorkspacesDelegate {
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
                Section { label, workspaces }
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
        Some(
            ListItem::new(("workspace", ix.section * 10_000 + ix.row))
                .selected(is_active)
                .child(ws.label.clone())
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
        Some(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(s.label.clone()),
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

pub struct LeftSidebarPanel {
    pub state: Entity<AppState>,
    list: Entity<ListState<WorkspacesDelegate>>,
    focus_handle: FocusHandle,
    _state_sub: Subscription,
    _store_sub: Subscription,
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
        Self {
            state,
            list: list_state,
            focus_handle: cx.focus_handle(),
            _state_sub: state_sub,
            _store_sub: store_sub,
        }
    }

    fn open_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.handle_picked(path, cx));
        })
        .detach();
    }

    fn new_worktree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let task = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = task.await else {
                return;
            };
            let Some(target) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                let repo = match workspace::validate_repo(&active) {
                    Ok(r) => r,
                    Err(err) => {
                        tracing::warn!(?err, "active workspace is not a git repo");
                        return;
                    }
                };
                let branch = target
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "swrm-wt".into());
                if let Err(err) = workspace::create_worktree(&repo, &branch, &target) {
                    tracing::warn!(?err, "create_worktree failed");
                    return;
                }
                this.handle_picked(target, cx);
            });
        })
        .detach();
    }

    fn handle_picked(&mut self, path: PathBuf, cx: &mut Context<Self>) {
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
        self.state.update(cx, |state, cx| {
            state.workspaces.update(cx, |store, cx| store.add(ws, cx));
            state.set_active_workspace(Some(path), cx);
        });
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
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("open-workspace")
                            .label("Open workspace\u{2026}")
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.open_workspace(window, cx);
                            })),
                    )
                    .child(
                        Button::new("new-worktree")
                            .label("New worktree\u{2026}")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.new_worktree(window, cx)),
                            ),
                    ),
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
