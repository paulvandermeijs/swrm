use swrm::app_state::AppState;
use gpui::{App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::dock::{Panel, PanelEvent};

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
}

impl Render for LeftSidebarPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            .track_focus(&self.focus_handle)
            .child("Workspaces")
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
