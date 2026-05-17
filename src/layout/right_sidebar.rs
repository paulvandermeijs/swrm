use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, Styled, Subscription, Window, div,
};
use gpui_component::button::Button;
use gpui_component::dock::{Panel, PanelEvent};
use std::path::PathBuf;
use swrm::app_state::{AppEvent, AppState};
use swrm::git::diff::diff_file;
use swrm::git::{Status, StatusEntry, collect_status};

pub struct RightSidebarPanel {
    pub state: Entity<AppState>,
    pub entries: Vec<StatusEntry>,
    pub selected: Option<PathBuf>,
    pub diff_text: String,
    focus_handle: FocusHandle,
    _sub: Subscription,
}

impl RightSidebarPanel {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&state, |this, _state, event, cx| {
            if matches!(event, AppEvent::ActiveWorkspaceChanged) {
                this.refresh(cx);
            }
        });
        let mut me = Self {
            state,
            entries: Vec::new(),
            selected: None,
            diff_text: String::new(),
            focus_handle: cx.focus_handle(),
            _sub: sub,
        };
        me.refresh(cx);
        me
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.read(cx).active_workspace.clone() else {
            self.entries.clear();
            self.selected = None;
            self.diff_text.clear();
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let entries = cx
                .background_executor()
                .spawn(async move { collect_status(&path) })
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(?err, "status failed");
                    Vec::new()
                });
            let _ = this.update(cx, |this, cx| {
                this.entries = entries;
                cx.notify();
            });
        })
        .detach();
    }

    fn select(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(repo) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let target = path.clone();
        cx.spawn(async move |this, cx| {
            let text = cx
                .background_executor()
                .spawn(async move { diff_file(&repo, &target) })
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(?err, "diff failed");
                    String::new()
                });
            let _ = this.update(cx, |this, cx| {
                this.selected = Some(path);
                this.diff_text = text;
                cx.notify();
            });
        })
        .detach();
    }
}

impl Focusable for RightSidebarPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RightSidebarPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        if self.entries.is_empty() {
            list = list.child("No changes");
        }
        for (idx, entry) in self.entries.iter().enumerate() {
            let badge = match entry.status {
                Status::Modified => "M",
                Status::Added => "A",
                Status::Deleted => "D",
                Status::Renamed => "R",
                Status::Untracked => "?",
            };
            let path = entry.path.clone();
            let is_selected = self.selected.as_ref() == Some(&entry.path);
            list = list.child(
                div()
                    .id(("status-entry", idx))
                    .flex()
                    .flex_row()
                    .gap_2()
                    .px_1()
                    .when(is_selected, |d| d.bg(gpui::rgb(0x3a3a3a)))
                    .child(div().w_4().child(badge))
                    .child(entry.path.display().to_string())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.select(path.clone(), cx);
                        }),
                    ),
            );
        }

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                Button::new("refresh")
                    .label("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(list)
            .child(div().mt_2().text_xs().child(self.diff_text.clone()))
    }
}

impl gpui::EventEmitter<PanelEvent> for RightSidebarPanel {}

impl Panel for RightSidebarPanel {
    fn panel_name(&self) -> &'static str {
        "right-sidebar"
    }

    fn zoomable(&self, _cx: &App) -> Option<gpui_component::dock::PanelControl> {
        None
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Changes"
    }
}
