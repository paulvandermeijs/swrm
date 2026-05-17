use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, PathPromptOptions, Render, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::button::Button;
use gpui_component::dock::{Panel, PanelEvent};
use std::path::PathBuf;
use swrm::app_state::AppState;
use swrm::workspace::{self, Workspace};

pub struct LeftSidebarPanel {
    pub state: Entity<AppState>,
    focus_handle: FocusHandle,
}

impl LeftSidebarPanel {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
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
            let Ok(Ok(Some(paths))) = task.await else { return };
            let Some(target) = paths.into_iter().next() else { return };
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
        let label = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        let ws = Workspace {
            label,
            path: path.clone(),
            branch,
        };
        self.state.update(cx, |state, cx| {
            state.workspaces.update(cx, |store, cx| store.add(ws, cx));
            state.set_active_workspace(Some(path), cx);
        });
    }
}

impl Render for LeftSidebarPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspaces = self.state.read(cx).workspaces.read(cx).workspaces.clone();
        let active = self.state.read(cx).active_workspace.clone();

        let mut list = div().flex().flex_col().gap_1();
        for ws in &workspaces {
            let is_active = active.as_ref() == Some(&ws.path);
            let path = ws.path.clone();
            let state = self.state.clone();
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .when(is_active, |d| d.bg(gpui::rgb(0x3a3a3a)))
                    .child(ws.label.clone())
                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                        state.update(cx, |s, cx| {
                            s.set_active_workspace(Some(path.clone()), cx);
                        });
                    }),
            );
        }

        div()
            .flex()
            .flex_col()
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
            .child(
                Button::new("new-worktree")
                    .label("New worktree\u{2026}")
                    .on_click(cx.listener(|this, _, window, cx| this.new_worktree(window, cx))),
            )
            .child(list)
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

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Workspaces"
    }
}
