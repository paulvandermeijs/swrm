use crate::layout::{self, Layout};
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, actions, div, rgb,
};
use gpui_component::dock::DockPlacement;
use swrm::app_state::AppState;

actions!(
    swrm,
    [
        ToggleLeftSidebar,
        ToggleRightSidebar,
        NewTab,
        CloseTab,
        SelectTab1,
        SelectTab2,
        SelectTab3,
        SelectTab4,
        SelectTab5,
        SelectTab6,
        SelectTab7,
        SelectTab8,
        SelectTab9,
    ]
);

pub struct Root {
    pub state: Entity<AppState>,
    pub layout: Layout,
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| AppState::new(cx));
        let layout = layout::build(state.clone(), window, cx);
        Self { state, layout }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let dock = self.layout.dock.clone();
        let tabs = self.layout.tabs.clone();
        let state = self.state.clone();

        div()
            .key_context("Root")
            .on_action({
                let dock = dock.clone();
                let state = state.clone();
                move |_: &ToggleLeftSidebar, window: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| s.toggle_left_sidebar(cx));
                    dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
                }
            })
            .on_action({
                let dock = dock.clone();
                let state = state.clone();
                move |_: &ToggleRightSidebar, window: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| s.toggle_right_sidebar(cx));
                    dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Right, window, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &NewTab, window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_new_tab(window, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &CloseTab, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_close_tab(cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab1, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(0, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab2, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(1, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab3, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(2, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab4, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(3, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab5, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(4, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab6, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(5, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab7, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(6, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab8, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(7, cx));
                }
            })
            .on_action({
                let tabs = tabs.clone();
                move |_: &SelectTab9, _window: &mut Window, cx: &mut App| {
                    tabs.update(cx, |t, cx| t.cmd_select_tab(8, cx));
                }
            })
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child(self.layout.dock.clone())
    }
}
