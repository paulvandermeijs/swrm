use crate::app_state::AppState;
use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, rgb};

pub struct Root {
    pub state: Entity<AppState>,
}

impl Root {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| AppState::new());
        Self { state }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child("swrm — state ready")
    }
}
