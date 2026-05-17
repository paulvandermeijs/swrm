use crate::app_state::AppState;
use crate::layout;
use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, rgb};
use gpui_component::dock::DockArea;

pub struct Root {
    pub state: Entity<AppState>,
    pub dock: Entity<DockArea>,
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| AppState::new());
        let dock = cx.new(|cx| layout::build(state.clone(), window, cx));
        Self { state, dock }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child(self.dock.clone())
    }
}
