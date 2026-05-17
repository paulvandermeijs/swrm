use crate::app_state::AppState;
use gpui::{App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::dock::{Panel, PanelEvent};

pub struct MainTabsPanel {
    pub state: Entity<AppState>,
    focus_handle: FocusHandle,
}

impl MainTabsPanel {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for MainTabsPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            .track_focus(&self.focus_handle)
            .child("Tabs go here")
    }
}

impl gpui::EventEmitter<PanelEvent> for MainTabsPanel {}

impl Focusable for MainTabsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for MainTabsPanel {
    fn panel_name(&self) -> &'static str {
        "main-tabs"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Terminal"
    }
}
