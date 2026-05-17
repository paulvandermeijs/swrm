use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Task, Window, div,
};
use gpui_component::button::Button;
use gpui_component::dock::{Panel, PanelEvent};
use gpui_component::list::{List, ListDelegate, ListItem, ListState};
use gpui_component::{ActiveTheme, IndexPath, h_flex, v_flex};
use std::path::PathBuf;
use swrm::app_state::{AppEvent, AppState};
use swrm::git::diff::diff_file;
use swrm::git::{Status, StatusEntry, collect_status};

pub struct RightSidebarPanel {
    pub state: Entity<AppState>,
    list: Entity<ListState<StatusDelegate>>,
    focus_handle: FocusHandle,
    _sub: Subscription,
}

impl RightSidebarPanel {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| {
            let delegate = StatusDelegate::new(state.clone());
            ListState::new(delegate, window, cx).selectable(true)
        });
        let sub = cx.subscribe(&state, |this, _state, event, cx| {
            if matches!(event, AppEvent::ActiveWorkspaceChanged) {
                this.refresh(cx);
            }
        });
        let mut me = Self {
            state,
            list,
            focus_handle: cx.focus_handle(),
            _sub: sub,
        };
        me.refresh(cx);
        me
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.read(cx).active_workspace.clone() else {
            self.list.update(cx, |list, cx| {
                let delegate = list.delegate_mut();
                delegate.sections.clear();
                delegate.selected = None;
                delegate.diff_text.clear();
                cx.notify();
            });
            return;
        };
        let list = self.list.clone();
        cx.spawn(async move |_this, cx| {
            let entries = cx
                .background_executor()
                .spawn(async move { collect_status(&path) })
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(?err, "status failed");
                    Vec::new()
                });
            let _ = list.update(cx, |list, cx| {
                let delegate = list.delegate_mut();
                delegate.sections = build_sections(entries);
                delegate.selected = None;
                delegate.diff_text.clear();
                cx.notify();
            });
        })
        .detach();
    }
}

impl Focusable for RightSidebarPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RightSidebarPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let diff_text = self.list.read(cx).delegate().diff_text.clone();
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .p_2()
            .gap_2()
            .child(
                Button::new("refresh")
                    .label("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(List::new(&self.list).into_any_element()),
            )
            .child(div().text_xs().child(diff_text))
    }
}

impl gpui::EventEmitter<PanelEvent> for RightSidebarPanel {}

impl Panel for RightSidebarPanel {
    fn panel_name(&self) -> &'static str {
        "right-sidebar"
    }

    fn zoomable(&self, _cx: &App) -> Option<gpui_component::dock::PanelControl> {
        None
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Changes"
    }
}

struct Section {
    label: SharedString,
    entries: Vec<StatusEntry>,
}

struct StatusDelegate {
    state: Entity<AppState>,
    sections: Vec<Section>,
    selected: Option<PathBuf>,
    diff_text: String,
    selected_index: Option<IndexPath>,
}

impl StatusDelegate {
    fn new(state: Entity<AppState>) -> Self {
        Self {
            state,
            sections: Vec::new(),
            selected: None,
            diff_text: String::new(),
            selected_index: None,
        }
    }

    fn entry_at(&self, ix: IndexPath) -> Option<&StatusEntry> {
        self.sections.get(ix.section)?.entries.get(ix.row)
    }

    fn load_diff(&mut self, path: PathBuf, cx: &mut Context<ListState<Self>>) {
        let Some(repo) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let target = path.clone();
        cx.spawn(async move |list, cx| {
            let text = cx
                .background_executor()
                .spawn(async move { diff_file(&repo, &target) })
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(?err, "diff failed");
                    String::new()
                });
            let _ = list.update(cx, |list, cx| {
                let delegate = list.delegate_mut();
                delegate.selected = Some(path);
                delegate.diff_text = text;
                cx.notify();
            });
        })
        .detach();
    }
}

impl ListDelegate for StatusDelegate {
    type Item = ListItem;

    fn sections_count(&self, _cx: &App) -> usize {
        self.sections.len().max(1)
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        self.sections
            .get(section)
            .map(|s| s.entries.len())
            .unwrap_or(0)
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let entry = self.entry_at(ix)?;
        let badge = match entry.status {
            Status::Modified => "M",
            Status::Added => "A",
            Status::Deleted => "D",
            Status::Renamed => "R",
            Status::Untracked => "?",
        };
        let is_selected = self.selected.as_ref() == Some(&entry.path);
        let display = entry.path.display().to_string();
        let row_id = ix.section * 10_000 + ix.row;
        Some(
            ListItem::new(("status-entry", row_id))
                .selected(is_selected)
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().w_4().child(badge))
                        .child(display),
                ),
        )
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let s = self.sections.get(section)?;
        let count = s.entries.len();
        let label = s.label.clone();
        Some(
            h_flex()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{count}")),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .justify_center()
            .items_center()
            .text_color(cx.theme().muted_foreground)
            .child("No changes")
    }

    fn perform_search(
        &mut self,
        _query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.selected_index else {
            return;
        };
        let Some(entry) = self.entry_at(ix) else {
            return;
        };
        let path = entry.path.clone();
        self.load_diff(path, cx);
    }
}

fn build_sections(entries: Vec<StatusEntry>) -> Vec<Section> {
    let mut changed = Vec::new();
    let mut untracked = Vec::new();
    for entry in entries {
        if matches!(entry.status, Status::Untracked) {
            untracked.push(entry);
        } else {
            changed.push(entry);
        }
    }
    let mut sections = Vec::new();
    if !changed.is_empty() {
        sections.push(Section {
            label: "Changes".into(),
            entries: changed,
        });
    }
    if !untracked.is_empty() {
        sections.push(Section {
            label: "Untracked".into(),
            entries: untracked,
        });
    }
    sections
}
