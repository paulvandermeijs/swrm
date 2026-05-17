use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollWheelEvent, Styled, Subscription, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dock::{Panel, PanelEvent};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{IconName, Sizable};
use std::collections::HashMap;
use std::path::PathBuf;
use swrm::app_state::{AppEvent, AppState};
use swrm::terminal::{
    Terminal, input,
    render::{CELL_FONT_SIZE_PX, CELL_LINE_HEIGHT_PX, render_snapshot},
};

pub struct TerminalTab {
    pub label: String,
    pub terminal: Terminal,
    pub focus: FocusHandle,
    pub exited: bool,
}

impl TerminalTab {
    pub fn new(label: String, cwd: PathBuf, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let mut terminal = Terminal::spawn(&cwd, 80, 24)?;
        let events = terminal
            .take_events()
            .expect("Terminal::spawn populates the event channel");
        let focus = cx.focus_handle();

        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            let mut events = events;
            while let Some(event) = events.next().await {
                use alacritty_terminal::event::Event;
                let cont = this
                    .update(cx, |this, cx| match event {
                        Event::Wakeup | Event::Bell | Event::MouseCursorDirty => {
                            cx.notify();
                            true
                        }
                        Event::Title(_) | Event::ResetTitle => true,
                        Event::ChildExit(_) | Event::Exit => {
                            this.exited = true;
                            cx.notify();
                            false
                        }
                        _ => true,
                    })
                    .ok()
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        })
        .detach();

        Ok(Self {
            label,
            terminal,
            focus,
            exited: false,
        })
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.exited {
            return;
        }
        if let Some(bytes) = input::encode(event, self.terminal.mode()) {
            if let Err(err) = self.terminal.write_input(&bytes) {
                tracing::warn!(?err, "pty write failed");
            }
            cx.notify();
        }
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Approximate line height in px, matching the renderer's text_size(px(13.)).
        let line_height_px: f32 = 18.0;
        let delta_y = event.delta.pixel_delta(gpui::px(line_height_px)).y.as_f32();
        let lines = (delta_y / line_height_px) as i32;
        if lines == 0 {
            return;
        }
        self.terminal.scroll(lines);
        cx.notify();
    }
}

impl Focusable for TerminalTab {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.terminal.snapshot();

        // Measure cell width using the same font + size the renderer paints with.
        let font_id = cx.text_system().resolve_font(&gpui::Font {
            family: "JetBrains Mono".into(),
            ..Default::default()
        });
        let cell_width = cx
            .text_system()
            .advance(font_id, gpui::px(CELL_FONT_SIZE_PX), 'm')
            .map(|s| s.width.as_f32())
            .unwrap_or(CELL_FONT_SIZE_PX * 0.6);
        let line_height = CELL_LINE_HEIGHT_PX;
        let entity = cx.entity().downgrade();

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .size_full()
            .bg(gpui::rgb(0x111111))
            .p_2()
            .child(
                div()
                    .size_full()
                    .relative()
                    .child(render_snapshot(&snap))
                    .child(
                        gpui::canvas(
                            move |bounds, _window, cx| {
                                let cols = ((bounds.size.width.as_f32() / cell_width).floor()
                                    as u16)
                                    .max(2);
                                let rows = ((bounds.size.height.as_f32() / line_height).floor()
                                    as u16)
                                    .max(1);
                                let _ = entity.update(cx, |this, _cx| {
                                    let _ = this.terminal.resize(cols, rows);
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    ),
            )
    }
}

#[derive(Default)]
struct WorkspaceTabs {
    tabs: Vec<Entity<TerminalTab>>,
    active_index: usize,
}

pub struct MainTabsPanel {
    pub state: Entity<AppState>,
    by_workspace: HashMap<PathBuf, WorkspaceTabs>,
    pub focus: FocusHandle,
    _sub: Subscription,
}

impl MainTabsPanel {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&state, |this, _state, event, cx| {
            if matches!(event, AppEvent::ActiveWorkspaceChanged) {
                this.ensure_tab_for_active(cx);
                cx.notify();
            }
        });
        Self {
            state,
            by_workspace: HashMap::new(),
            focus: cx.focus_handle(),
            _sub: sub,
        }
    }

    fn ensure_tab_for_active(&mut self, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        if self
            .by_workspace
            .get(&cwd)
            .map(|w| !w.tabs.is_empty())
            .unwrap_or(false)
        {
            return;
        }
        let tab = cx.new(|cx| TerminalTab::new("terminal 1".into(), cwd.clone(), cx).unwrap());
        let entry = self.by_workspace.entry(cwd).or_default();
        entry.tabs.push(tab);
        entry.active_index = 0;
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let entry = self.by_workspace.entry(cwd.clone()).or_default();
        let label = format!("terminal {}", entry.tabs.len() + 1);
        let tab = cx.new(|cx| TerminalTab::new(label, cwd, cx).unwrap());
        entry.tabs.push(tab);
        entry.active_index = entry.tabs.len() - 1;
        cx.notify();
    }

    fn close_active(&mut self, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let Some(entry) = self.by_workspace.get(&cwd) else {
            return;
        };
        let idx = entry.active_index;
        self.close_at(idx, cx);
    }

    fn close_at(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let Some(entry) = self.by_workspace.get_mut(&cwd) else {
            return;
        };
        if idx >= entry.tabs.len() {
            return;
        }
        entry.tabs.remove(idx);
        if entry.active_index >= entry.tabs.len() && entry.active_index > 0 {
            entry.active_index = entry.tabs.len().saturating_sub(1);
        }
        cx.notify();
    }

    fn select(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let Some(entry) = self.by_workspace.get_mut(&cwd) else {
            return;
        };
        if idx < entry.tabs.len() {
            entry.active_index = idx;
            cx.notify();
        }
    }

    pub fn cmd_new_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.new_tab(cx);
    }

    pub fn cmd_close_tab(&mut self, cx: &mut Context<Self>) {
        self.close_active(cx);
    }

    pub fn cmd_select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.select(idx, cx);
    }
}

impl Focusable for MainTabsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for MainTabsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_workspace = self.state.read(cx).active_workspace.clone();
        let entry = active_workspace
            .as_ref()
            .and_then(|p| self.by_workspace.get(p));

        let tab_labels: Vec<String> = entry
            .map(|e| e.tabs.iter().map(|t| t.read(cx).label.clone()).collect())
            .unwrap_or_default();
        let selected_index = entry.map(|e| e.active_index).unwrap_or(0);
        let active_tab = entry.and_then(|e| e.tabs.get(e.active_index)).cloned();
        let has_workspace = active_workspace.is_some();

        let mut bar = TabBar::new("main-tabs-bar")
            .w_full()
            .selected_index(selected_index)
            .on_click(cx.listener(|this, ix: &usize, _window, cx| this.select(*ix, cx)));

        for (idx, label) in tab_labels.into_iter().enumerate() {
            bar = bar.child(
                Tab::new().label(label).suffix(
                    Button::new(("close-tab", idx))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _, _, cx| this.close_at(idx, cx))),
                ),
            );
        }

        if has_workspace {
            bar = bar.suffix(
                Button::new("new-tab")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, _, cx| this.new_tab(cx))),
            );
        }

        div()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .child(bar)
            .when_some(active_tab, |d, tab| d.child(tab))
    }
}

impl gpui::EventEmitter<PanelEvent> for MainTabsPanel {}

impl Panel for MainTabsPanel {
    fn panel_name(&self) -> &'static str {
        "main-tabs"
    }

    fn zoomable(&self, _cx: &App) -> Option<gpui_component::dock::PanelControl> {
        None
    }

    fn title(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (active_path, store) = {
            let s = self.state.read(cx);
            (s.active_workspace.clone(), s.workspaces.clone())
        };
        let title = active_path
            .as_ref()
            .and_then(|path| {
                store
                    .read(cx)
                    .workspaces
                    .iter()
                    .find(|w| &w.path == path)
                    .map(|ws| match &ws.branch {
                        Some(b) => format!("{} \u{2014} {}", ws.label, b),
                        None => ws.label.clone(),
                    })
            })
            .unwrap_or_else(|| "Terminal".to_string());
        title
    }
}
