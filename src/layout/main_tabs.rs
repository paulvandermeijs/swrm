use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, Styled, Subscription, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::dock::{Panel, PanelEvent};
use std::path::PathBuf;
use std::time::Duration;
use swrm::app_state::{AppEvent, AppState};
use swrm::terminal::{Terminal, input, render::render_snapshot};

pub struct TerminalTab {
    pub label: String,
    pub terminal: Terminal,
    pub focus: FocusHandle,
}

impl TerminalTab {
    pub fn new(label: String, cwd: PathBuf, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let terminal = Terminal::spawn(&cwd, 80, 24)?;
        let focus = cx.focus_handle();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let cont = this.update(cx, |this, cx| {
                    let changed = this.terminal.tick();
                    if changed {
                        cx.notify();
                    }
                    !this
                        .terminal
                        .pty
                        .child
                        .try_wait()
                        .map(|s| s.is_some())
                        .unwrap_or(false)
                });
                if !matches!(cont, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        Ok(Self { label, terminal, focus })
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bytes) = input::encode(event) {
            if let Err(err) = self.terminal.write_input(&bytes) {
                tracing::warn!(?err, "pty write failed");
            }
            cx.notify();
        }
    }
}

impl Focusable for TerminalTab {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.terminal.snapshot().ok();
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .bg(gpui::rgb(0x111111))
            .p_2()
            .when_some(snap, |d, snap| d.child(render_snapshot(&snap)))
    }
}

pub struct MainTabsPanel {
    pub state: Entity<AppState>,
    pub tabs: Vec<Entity<TerminalTab>>,
    pub active_index: usize,
    pub focus: FocusHandle,
    _sub: Subscription,
}

impl MainTabsPanel {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&state, |this, _state, event, cx| {
            if matches!(event, AppEvent::ActiveWorkspaceChanged) {
                this.spawn_initial_tab(cx);
            }
        });
        Self {
            state,
            tabs: Vec::new(),
            active_index: 0,
            focus: cx.focus_handle(),
            _sub: sub,
        }
    }

    fn spawn_initial_tab(&mut self, cx: &mut Context<Self>) {
        if !self.tabs.is_empty() {
            return;
        }
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let tab = cx.new(|cx| TerminalTab::new("terminal 1".into(), cwd, cx).unwrap());
        self.tabs.push(tab);
        self.active_index = 0;
        cx.notify();
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let label = format!("terminal {}", self.tabs.len() + 1);
        let tab = cx.new(|cx| TerminalTab::new(label, cwd, cx).unwrap());
        self.tabs.push(tab);
        self.active_index = self.tabs.len() - 1;
        cx.notify();
    }

    fn close_active(&mut self, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        self.tabs.remove(self.active_index);
        if self.active_index >= self.tabs.len() && self.active_index > 0 {
            self.active_index -= 1;
        }
        cx.notify();
    }

    pub fn cmd_new_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.new_tab(cx);
    }

    pub fn cmd_close_tab(&mut self, cx: &mut Context<Self>) {
        self.close_active(cx);
    }

    pub fn cmd_select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() {
            self.active_index = idx;
            cx.notify();
        }
    }
}

impl Focusable for MainTabsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for MainTabsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div().flex().flex_row().gap_2().p_2();
        for (idx, tab) in self.tabs.iter().enumerate() {
            let label = tab.read(cx).label.clone();
            let is_active = idx == self.active_index;
            bar = bar.child(
                div()
                    .px_2()
                    .py_1()
                    .when(is_active, |d| d.bg(gpui::rgb(0x3a3a3a)))
                    .child(label)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.active_index = idx;
                        cx.notify();
                    })),
            );
        }
        let active = self.tabs.get(self.active_index).cloned();

        div()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .child(bar)
            .when_some(active, |d, tab| d.child(tab))
    }
}

impl gpui::EventEmitter<PanelEvent> for MainTabsPanel {}

impl Panel for MainTabsPanel {
    fn panel_name(&self) -> &'static str {
        "main-tabs"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Terminal"
    }
}
