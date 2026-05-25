use crate::layout::{self, Layout};
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, actions, div,
};
use gpui_component::ActiveTheme;
use gpui_component::Root as ComponentRoot;
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
        // Bound in the "Terminal" key context so that they outrank gpui-component's
        // Root-context tab/shift-tab focus-navigation bindings when the terminal
        // is focused. The TerminalTab handlers write the appropriate bytes to the PTY.
        SendTerminalTab,
        SendTerminalShiftTab,
        OpenSettings,
        ShowAbout,
        Quit,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dock = self.layout.dock.clone();
        let tabs = self.layout.tabs.clone();
        let state = self.state.clone();

        // Mirror what gpui_component::Root does: apply theme font/colors to the root div.
        window.set_rem_size(cx.theme().font_size);

        div()
            .key_context("Root")
            .font_family(cx.theme().font_family.clone())
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
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
            .on_action(|_: &OpenSettings, window: &mut Window, cx: &mut App| {
                gpui_component::Root::update(window, cx, |root, window, cx| {
                    root.open_dialog(
                        |dialog, _window, _cx| {
                            dialog
                                .title("Settings")
                                .content(|content, _, _| content)
                                .w(gpui::px(800.))
                                .close_button(true)
                                .overlay_closable(true)
                                .keyboard(true)
                        },
                        window,
                        cx,
                    );
                });
            })
            .on_action(|_: &ShowAbout, _window: &mut Window, _cx: &mut App| {
                tracing::info!("swrm v{}", env!("CARGO_PKG_VERSION"));
            })
            .on_action(|_: &Quit, _window: &mut Window, cx: &mut App| {
                cx.quit();
            })
            .size_full()
            .child(self.layout.dock.clone())
            .children(ComponentRoot::render_sheet_layer(window, cx))
            .children(ComponentRoot::render_dialog_layer(window, cx))
            .children(ComponentRoot::render_notification_layer(window, cx))
    }
}
