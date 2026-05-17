use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Subscription, Window, div,
};
use gpui_component::dock::{Panel, PanelEvent};
use swrm::app_state::{AppEvent, AppState};
use swrm::git::{Status, StatusEntry, collect_status};

pub struct RightSidebarPanel {
    pub state: Entity<AppState>,
    pub entries: Vec<StatusEntry>,
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
            focus_handle: cx.focus_handle(),
            _sub: sub,
        };
        me.refresh(cx);
        me
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.read(cx).active_workspace.clone() else {
            self.entries.clear();
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
}

impl Focusable for RightSidebarPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RightSidebarPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        if self.entries.is_empty() {
            list = list.child("No changes");
        }
        for entry in &self.entries {
            let badge = match entry.status {
                Status::Modified => "M",
                Status::Added => "A",
                Status::Deleted => "D",
                Status::Renamed => "R",
                Status::Untracked => "?",
            };
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().w_4().child(badge))
                    .child(entry.path.display().to_string()),
            );
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            .track_focus(&self.focus_handle)
            .child(list)
    }
}

impl gpui::EventEmitter<PanelEvent> for RightSidebarPanel {}

impl Panel for RightSidebarPanel {
    fn panel_name(&self) -> &'static str {
        "right-sidebar"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Changes"
    }
}
