use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, rgb};

pub struct Root;

impl Root {
    pub fn new() -> Self {
        Self
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child("swrm")
    }
}
