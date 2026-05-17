use swrm::app_state::AppState;
use crate::layout;
use gpui::{App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, actions, div, rgb};
use gpui_component::dock::DockArea;

actions!(swrm, [ToggleLeftSidebar, ToggleRightSidebar]);

pub struct Root {
    pub state: Entity<AppState>,
    pub dock: Entity<DockArea>,
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| AppState::new(cx));
        let dock = cx.new(|cx| layout::build(state.clone(), window, cx));
        Self { state, dock }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let dock = self.dock.clone();
        let state = self.state.clone();
        div()
            .key_context("Root")
            .on_action({
                let dock = dock.clone();
                let state = state.clone();
                move |_: &ToggleLeftSidebar, window: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| s.toggle_left_sidebar(cx));
                    dock.update(cx, |d, cx| {
                        d.toggle_dock(gpui_component::dock::DockPlacement::Left, window, cx)
                    });
                }
            })
            .on_action({
                let dock = dock.clone();
                let state = state.clone();
                move |_: &ToggleRightSidebar, window: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| s.toggle_right_sidebar(cx));
                    dock.update(cx, |d, cx| {
                        d.toggle_dock(gpui_component::dock::DockPlacement::Right, window, cx)
                    });
                }
            })
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child(self.dock.clone())
    }
}
