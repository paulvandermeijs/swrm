# Configurable Agents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users define named agents (label + shell command) in a Settings UI, pick one when opening a new tab via the "+" dropdown, and auto-spawn the first agent as the default tab in every new workspace.

**Architecture:** New `settings` module (data model + JSON persistence + `SettingsStore` entity) mirrors the existing `workspace` module. A `TabSpec` enum threads through the tab-spawn boundary so `TerminalTab::new` can run either a plain shell or `$SHELL -c <agent.command>`. The "+" button becomes a `dropdown_menu` built from `SettingsStore::agents()` on each render. Settings opens via a native macOS menu (⌘,) into a gpui-component Dialog.

**Tech Stack:** Rust, `gpui`, `gpui-component` (Settings/Dialog/PopupMenu/Input/Button), `alacritty_terminal`, `serde_json`. No new crates — `serde`, `serde_json`, `anyhow`, `dirs`, `tracing`, `tempfile` are already pulled in.

**Spec:** `docs/superpowers/specs/2026-05-25-configurable-agents-design.md`

---

## Task 1: Settings module — data model and persistence

**Files:**
- Create: `src/settings/mod.rs`
- Create: `src/settings/persistence.rs`
- Modify: `src/lib.rs:1-5`
- Test: `tests/settings_persistence.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/settings_persistence.rs`:

```rust
use swrm::settings::{Agent, AppSettings, persistence};
use tempfile::tempdir;

#[test]
fn round_trips_agents() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let settings = AppSettings {
        agents: vec![
            Agent {
                id: "agent-1".into(),
                name: "claude".into(),
                command: "claude".into(),
            },
            Agent {
                id: "agent-2".into(),
                name: "codex".into(),
                command: "codex --print".into(),
            },
        ],
    };
    persistence::save_to(&path, &settings).unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(loaded, settings);
}

#[test]
fn missing_file_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.json");
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(loaded, AppSettings::default());
}

#[test]
fn malformed_json_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, b"not json at all").unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(
        loaded,
        AppSettings::default(),
        "malformed JSON should fall back to default"
    );
}

#[test]
fn missing_agents_key_loads_as_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.json");
    std::fs::write(&path, b"{}").unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(loaded, AppSettings::default());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test settings_persistence`
Expected: compile error — `unresolved import \`swrm::settings\``.

- [ ] **Step 3: Create `src/settings/mod.rs`**

```rust
pub mod persistence;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default)]
    pub agents: Vec<Agent>,
}
```

- [ ] **Step 4: Create `src/settings/persistence.rs`**

```rust
use super::AppSettings;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("no config dir on this platform")?;
    Ok(dir.join("swrm").join("settings.json"))
}

pub fn load_from(path: &Path) -> Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    match serde_json::from_str::<AppSettings>(&raw) {
        Ok(settings) => Ok(settings),
        Err(err) => {
            tracing::warn!(
                ?err,
                path = %path.display(),
                "malformed settings.json, falling back to default"
            );
            Ok(AppSettings::default())
        }
    }
}

pub fn save_to(path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(path, raw)?;
    Ok(())
}

pub fn load() -> Result<AppSettings> {
    load_from(&config_path()?)
}

pub fn save(settings: &AppSettings) -> Result<()> {
    save_to(&config_path()?, settings)
}
```

- [ ] **Step 5: Register the module in `src/lib.rs`**

Add `pub mod settings;` after the existing `pub mod` lines:

```rust
pub mod app_state;
pub mod git;
pub mod settings;
pub mod terminal;
pub mod workspace;
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --test settings_persistence`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 7: Format**

Run: `cargo fmt`

- [ ] **Step 8: Commit**

```bash
git add src/lib.rs src/settings/ tests/settings_persistence.rs
git commit -m "feat: Add settings persistence with Agent + AppSettings model"
```

---

## Task 2: SettingsStore entity

**Files:**
- Create: `src/settings/store.rs`
- Modify: `src/settings/mod.rs`
- Test: `tests/settings_persistence.rs` (append id-generation cases)

- [ ] **Step 1: Append failing tests for `next_id`**

Append to `tests/settings_persistence.rs`:

```rust
use swrm::settings::store::next_id;

#[test]
fn next_id_starts_at_one() {
    assert_eq!(next_id(&[]), "agent-1");
}

#[test]
fn next_id_is_max_plus_one() {
    let agents = vec![
        Agent { id: "agent-1".into(), name: String::new(), command: String::new() },
        Agent { id: "agent-2".into(), name: String::new(), command: String::new() },
    ];
    assert_eq!(next_id(&agents), "agent-3");
}

#[test]
fn next_id_does_not_reset_into_gaps() {
    let agents = vec![
        Agent { id: "agent-1".into(), name: String::new(), command: String::new() },
        Agent { id: "agent-5".into(), name: String::new(), command: String::new() },
    ];
    assert_eq!(next_id(&agents), "agent-6");
}

#[test]
fn next_id_ignores_unparseable_ids() {
    let agents = vec![
        Agent { id: "weird".into(), name: String::new(), command: String::new() },
        Agent { id: "agent-2".into(), name: String::new(), command: String::new() },
    ];
    assert_eq!(next_id(&agents), "agent-3");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test settings_persistence`
Expected: compile error — `unresolved import \`swrm::settings::store\``.

- [ ] **Step 3: Create `src/settings/store.rs`**

```rust
use super::{Agent, persistence};
use gpui::{Context, EventEmitter};

#[derive(Clone, Debug)]
pub enum SettingsEvent {
    Changed,
}

#[derive(Clone, Copy, Debug)]
pub enum MoveDir {
    Up,
    Down,
}

pub struct SettingsStore {
    pub settings: super::AppSettings,
}

impl SettingsStore {
    pub fn load() -> Self {
        let settings = persistence::load().unwrap_or_else(|err| {
            tracing::warn!(?err, "failed to load settings, starting empty");
            super::AppSettings::default()
        });
        Self { settings }
    }

    pub fn agents(&self) -> &[Agent] {
        &self.settings.agents
    }

    pub fn add_agent(&mut self, cx: &mut Context<Self>) -> String {
        let id = next_id(&self.settings.agents);
        self.settings.agents.push(Agent {
            id: id.clone(),
            name: String::new(),
            command: String::new(),
        });
        self.persist();
        cx.emit(SettingsEvent::Changed);
        cx.notify();
        id
    }

    pub fn update_agent(
        &mut self,
        id: &str,
        name: String,
        command: String,
        cx: &mut Context<Self>,
    ) {
        let Some(agent) = self.settings.agents.iter_mut().find(|a| a.id == id) else {
            return;
        };
        if agent.name == name && agent.command == command {
            return;
        }
        agent.name = name;
        agent.command = command;
        self.persist();
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }

    pub fn remove_agent(&mut self, id: &str, cx: &mut Context<Self>) {
        let before = self.settings.agents.len();
        self.settings.agents.retain(|a| a.id != id);
        if self.settings.agents.len() != before {
            self.persist();
            cx.emit(SettingsEvent::Changed);
            cx.notify();
        }
    }

    pub fn move_agent(&mut self, id: &str, dir: MoveDir, cx: &mut Context<Self>) {
        let Some(idx) = self.settings.agents.iter().position(|a| a.id == id) else {
            return;
        };
        let target = match dir {
            MoveDir::Up if idx > 0 => idx - 1,
            MoveDir::Down if idx + 1 < self.settings.agents.len() => idx + 1,
            _ => return,
        };
        self.settings.agents.swap(idx, target);
        self.persist();
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }

    fn persist(&self) {
        if let Err(err) = persistence::save(&self.settings) {
            tracing::error!(?err, "failed to persist settings");
        }
    }
}

impl EventEmitter<SettingsEvent> for SettingsStore {}

pub fn next_id(agents: &[Agent]) -> String {
    let max = agents
        .iter()
        .filter_map(|a| {
            a.id.strip_prefix("agent-")
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    format!("agent-{}", max + 1)
}
```

- [ ] **Step 4: Re-export from `src/settings/mod.rs`**

At the top of `src/settings/mod.rs`, add `pub mod store;` and `pub use store::{MoveDir, SettingsEvent, SettingsStore};`:

```rust
pub mod persistence;
pub mod store;

pub use store::{MoveDir, SettingsEvent, SettingsStore};

use serde::{Deserialize, Serialize};
// (rest unchanged)
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test settings_persistence`
Expected: `test result: ok. 8 passed; 0 failed` (4 original + 4 new).

- [ ] **Step 6: Format and build**

Run: `cargo fmt && cargo build`
Expected: clean build with the pre-existing `dead_code` warning only.

- [ ] **Step 7: Commit**

```bash
git add src/settings/ tests/settings_persistence.rs
git commit -m "feat: Add SettingsStore entity with CRUD and stable agent ids"
```

---

## Task 3: `Terminal::spawn_command` for agent execution

**Files:**
- Modify: `src/terminal/backend.rs` (add `Backend::spawn_command`)
- Modify: `src/terminal/mod.rs` (add `Terminal::spawn_command`)
- Test: `tests/agent_command_spawn.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/agent_command_spawn.rs`:

```rust
use std::time::{Duration, Instant};
use swrm::terminal::Terminal;

/// Verifies that `Terminal::spawn_command` runs the given string through
/// `$SHELL -c` and emits its stdout to the PTY. Mirrors the polling pattern
/// used by `child_shell_sees_xterm_256color_term` in `vt_smoke.rs`.
#[test]
fn agent_command_runs_through_shell() {
    let cwd = std::env::temp_dir();
    let mut term = Terminal::spawn_command(&cwd, "printf 'AGENT=[%s]' yes", 80, 6)
        .expect("spawn agent command");
    let _ = term.take_events();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snap = term.snapshot();
        let grid: String = snap.cells.iter().map(|c| c.ch).collect();
        if grid.contains("AGENT=[yes]") {
            return;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for command output; grid = {grid:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test agent_command_spawn`
Expected: compile error — `no function or associated item named \`spawn_command\` found for struct \`Terminal\``.

- [ ] **Step 3: Add `Backend::spawn_command` in `src/terminal/backend.rs`**

Right after the existing `pub fn spawn(...)` block (ends around line 90), insert this new method. The body is identical except for the shell args:

```rust
    pub fn spawn_command(
        cwd: &Path,
        command: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(Self, UnboundedReceiver<AlacEvent>)> {
        let size = TermSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let (listener, events_rx) = SwrmListener::pair();
        let term = Term::new(Config::default(), &size, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut tty_opts = tty::Options::default();
        tty_opts.shell = Some(tty::Shell::new(
            shell,
            vec!["-c".into(), command.to_string()],
        ));
        tty_opts.working_directory = Some(cwd.to_path_buf());
        tty_opts.drain_on_exit = true;
        tty_opts.env = child_env();

        let pty = tty::new(&tty_opts, size.into(), 0).context("open pty + fork shell -c")?;

        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)
            .context("build alacritty event loop")?;

        let tx = event_loop.channel();
        let _join = Some(event_loop.spawn());

        Ok((
            Self {
                term,
                tx: Some(tx),
                _join,
                size,
            },
            events_rx,
        ))
    }
```

- [ ] **Step 4: Add `Terminal::spawn_command` in `src/terminal/mod.rs`**

Right after the existing `pub fn spawn(...)` block (ends around line 29), insert:

```rust
    pub fn spawn_command(cwd: &Path, command: &str, cols: u16, rows: u16) -> Result<Self> {
        let (backend, events) = Backend::spawn_command(cwd, command, cols, rows)?;
        Ok(Self {
            backend,
            events: Some(events),
        })
    }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test agent_command_spawn`
Expected: `test result: ok. 1 passed`. Takes up to a second (shell startup + output).

- [ ] **Step 6: Run all tests to confirm nothing else broke**

Run: `cargo test`
Expected: every previously-passing test still passes; one new test added.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt
git add src/terminal/backend.rs src/terminal/mod.rs tests/agent_command_spawn.rs
git commit -m 'feat: Add Terminal::spawn_command to launch tabs via $SHELL -c'
```

---

## Task 4: `unique_label` helper for tab disambiguation

**Files:**
- Create: `src/tab_labels.rs` (top-level leaf module — keeps the library free of UI deps)
- Modify: `src/lib.rs`
- Test: `tests/tab_labels.rs`

The helper lives directly under `src/` instead of `src/layout/` so we can expose it from the library crate without also exposing the UI-heavy `layout` module.

- [ ] **Step 1: Write the failing test**

Create `tests/tab_labels.rs`:

```rust
use swrm::tab_labels::unique_label;

#[test]
fn returns_base_when_no_existing() {
    assert_eq!(unique_label(&[], "claude"), "claude");
    assert_eq!(unique_label(&["other"], "claude"), "claude");
}

#[test]
fn appends_two_when_base_is_taken() {
    assert_eq!(unique_label(&["claude"], "claude"), "claude 2");
}

#[test]
fn fills_gaps_after_delete() {
    assert_eq!(unique_label(&["claude", "claude 3"], "claude"), "claude 2");
}

#[test]
fn continues_past_three() {
    assert_eq!(
        unique_label(&["claude", "claude 2", "claude 3"], "claude"),
        "claude 4"
    );
}

#[test]
fn different_kinds_are_independent() {
    assert_eq!(unique_label(&["terminal", "claude"], "terminal"), "terminal 2");
    assert_eq!(unique_label(&["terminal", "claude"], "claude"), "claude 2");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tab_labels`
Expected: compile error — `unresolved import \`swrm::tab_labels\``.

- [ ] **Step 3: Create `src/tab_labels.rs`**

```rust
/// Pick a tab label that doesn't collide with any name already in use within
/// a workspace. Returns `base` if it's free, otherwise the smallest
/// `format!("{base} {n}")` for `n >= 2` that isn't taken.
pub fn unique_label(existing: &[&str], base: &str) -> String {
    if !existing.iter().any(|s| *s == base) {
        return base.to_string();
    }
    for n in 2u32.. {
        let candidate = format!("{base} {n}");
        if !existing.iter().any(|s| *s == candidate) {
            return candidate;
        }
    }
    unreachable!("u32 exhausted while picking a tab label")
}
```

- [ ] **Step 4: Register in `src/lib.rs`**

```rust
pub mod app_state;
pub mod git;
pub mod settings;
pub mod tab_labels;
pub mod terminal;
pub mod workspace;
```

(Do NOT add `pub mod layout;` — that pulls in UI deps. `tab_labels` is a leaf.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test tab_labels`
Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 6: Format and commit**

```bash
cargo fmt
git add src/lib.rs src/tab_labels.rs tests/tab_labels.rs
git commit -m "feat: Add unique_label helper for tab name disambiguation"
```

---

## Task 5: Wire `SettingsStore` into `AppState`

**Files:**
- Modify: `src/app_state.rs`

No new tests — this is plumbing. Existing tests must keep passing.

- [ ] **Step 1: Update `AppState` to hold a `SettingsStore` entity**

Replace `src/app_state.rs` entirely with:

```rust
use crate::settings::SettingsStore;
use crate::workspace::WorkspaceStore;
use gpui::{AppContext, Context, Entity, EventEmitter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    ActiveWorkspaceChanged,
    SidebarToggled,
}

pub struct AppState {
    pub workspaces: Entity<WorkspaceStore>,
    pub settings: Entity<SettingsStore>,
    pub active_workspace: Option<PathBuf>,
    pub left_sidebar_visible: bool,
    pub right_sidebar_visible: bool,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let workspaces = cx.new(|cx| WorkspaceStore::load(cx));
        let settings = cx.new(|cx| SettingsStore::load(cx));
        Self {
            workspaces,
            settings,
            active_workspace: None,
            left_sidebar_visible: true,
            right_sidebar_visible: true,
        }
    }

    pub fn set_active_workspace(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.active_workspace != path {
            self.active_workspace = path;
            cx.emit(AppEvent::ActiveWorkspaceChanged);
            cx.notify();
        }
    }

    pub fn toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.left_sidebar_visible = !self.left_sidebar_visible;
        cx.emit(AppEvent::SidebarToggled);
        cx.notify();
    }

    pub fn toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.right_sidebar_visible = !self.right_sidebar_visible;
        cx.emit(AppEvent::SidebarToggled);
        cx.notify();
    }
}

impl EventEmitter<AppEvent> for AppState {}
```

- [ ] **Step 2: Build to confirm nothing else broke**

Run: `cargo build`
Expected: clean build (one pre-existing `dead_code` warning).

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add src/app_state.rs
git commit -m "feat: Hold SettingsStore alongside WorkspaceStore on AppState"
```

---

## Task 6: Application menu and `OpenSettings` action

**Files:**
- Modify: `src/app.rs`
- Modify: `src/main.rs`

Visible behavior: native macOS menu bar appears with `swrm` (About, Settings… ⌘,, Quit ⌘Q), File (New Tab ⌘T, Close Tab ⌘W), and View (Toggle Left/Right Sidebar). `About` logs the version; `Settings…` opens a placeholder Dialog ("Settings" title, empty body). Task 7 fills the body in.

- [ ] **Step 1: Add the new actions in `src/app.rs`**

Update the `actions!` block (top of file) — add `OpenSettings`, `ShowAbout`, `Quit`:

```rust
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
        SendTerminalTab,
        SendTerminalShiftTab,
        OpenSettings,
        ShowAbout,
        Quit,
    ]
);
```

- [ ] **Step 2: Handle the new actions in `Root::render`**

Inside `impl Render for Root`, after the existing `.on_action` blocks and before `.size_full()`, add three new handlers. Insert after the `SelectTab9` handler block (around line 142):

```rust
            .on_action({
                move |_: &OpenSettings, window: &mut Window, cx: &mut App| {
                    gpui_component::Root::update(window, cx, |root, window, cx| {
                        root.open_dialog(
                            move |dialog, _window, _cx| {
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
                }
            })
            .on_action(|_: &ShowAbout, _window: &mut Window, _cx: &mut App| {
                tracing::info!("swrm v{}", env!("CARGO_PKG_VERSION"));
            })
            .on_action(|_: &Quit, _window: &mut Window, cx: &mut App| {
                cx.quit();
            })
```

You may need to add `use gpui::App;` and adjust the existing `gpui::` import list — the `App` type comes from `gpui`. Verify imports compile after the edit.

- [ ] **Step 3: Bind keys and set menus in `src/main.rs`**

Add the new bindings to the existing `cx.bind_keys([...])` block (insert after the existing `cmd-9` line):

```rust
                gpui::KeyBinding::new("cmd-,", app::OpenSettings, Some("Root")),
                gpui::KeyBinding::new("cmd-q", app::Quit, Some("Root")),
```

(Keep the existing bindings; this just appends two.)

Then, immediately after the `cx.bind_keys(...)` call, add:

```rust
            cx.set_menus(vec![
                gpui::Menu {
                    name: "swrm".into(),
                    items: vec![
                        gpui::MenuItem::action("About swrm", app::ShowAbout),
                        gpui::MenuItem::Separator,
                        gpui::MenuItem::action("Settings…", app::OpenSettings),
                        gpui::MenuItem::Separator,
                        gpui::MenuItem::action("Quit swrm", app::Quit),
                    ],
                },
                gpui::Menu {
                    name: "File".into(),
                    items: vec![
                        gpui::MenuItem::action("New Tab", app::NewTab),
                        gpui::MenuItem::action("Close Tab", app::CloseTab),
                    ],
                },
                gpui::Menu {
                    name: "View".into(),
                    items: vec![
                        gpui::MenuItem::action("Toggle Left Sidebar", app::ToggleLeftSidebar),
                        gpui::MenuItem::action("Toggle Right Sidebar", app::ToggleRightSidebar),
                    ],
                },
            ]);
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean. If `gpui::Menu` / `MenuItem` aren't found, check whether they live under `gpui::platform::*` instead — adjust the import path. (They are re-exported at the gpui crate root in current gpui versions; this should just work.)

- [ ] **Step 5: Manual smoke check**

Run: `cargo run` (let it boot for ~3 seconds, then kill). Verify:
- No panics in the log.
- The macOS menu bar shows "swrm", "File", "View" entries (you can confirm by mouse-clicking the menu titles, or just by `cargo run` + screenshotting).
- ⌘, opens an empty Settings dialog (placeholder body, close button works).

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/app.rs src/main.rs
git commit -m "feat: Add native menu bar with Settings (⌘,) opening a placeholder dialog"
```

---

## Task 7: `SettingsView` modal with full agent CRUD

**Files:**
- Create: `src/layout/settings_view.rs`
- Modify: `src/layout/mod.rs` (`pub mod settings_view;`)
- Modify: `src/app.rs` (have `OpenSettings` mount the real view)

This is the chunky task. Build the entity in pieces; keep it compiling at each step.

- [ ] **Step 1: Scaffold `SettingsView` with empty render**

Create `src/layout/settings_view.rs`:

```rust
use std::collections::HashMap;

use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div,
};
use gpui_component::{
    IconName,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    setting::{SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use swrm::app_state::AppState;
use swrm::settings::{MoveDir, SettingsEvent, SettingsStore};

struct AgentInputs {
    name: Entity<InputState>,
    command: Entity<InputState>,
    _subs: Vec<Subscription>,
}

pub struct SettingsView {
    state: Entity<AppState>,
    inputs: HashMap<String, AgentInputs>,
    focus: FocusHandle,
    _store_sub: Subscription,
}

impl SettingsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = state.read(cx).settings.clone();
        let store_sub = cx.subscribe(&store, |this, _store, event, cx| {
            if matches!(event, SettingsEvent::Changed) {
                this.reconcile_inputs(cx);
                cx.notify();
            }
        });
        let mut view = Self {
            state,
            inputs: HashMap::new(),
            focus: cx.focus_handle(),
            _store_sub: store_sub,
        };
        view.reconcile_inputs_with_window(window, cx);
        view
    }

    fn reconcile_inputs(&mut self, cx: &mut Context<Self>) {
        // Called from observation context: no `window`. Defer creation to the
        // next render via `cx.spawn_in` only if needed — but `InputState::new`
        // requires a `Window`. Instead, mark dirty and create lazily in render.
        // Simpler path: skip here and rely on the next call to
        // `reconcile_inputs_with_window` from `render`.
        let _ = cx;
    }

    fn reconcile_inputs_with_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agents = self.state.read(cx).settings.read(cx).agents().to_vec();
        let live_ids: std::collections::HashSet<String> =
            agents.iter().map(|a| a.id.clone()).collect();
        self.inputs.retain(|id, _| live_ids.contains(id));

        for agent in &agents {
            if self.inputs.contains_key(&agent.id) {
                continue;
            }
            let name_state = cx.new(|cx| {
                let mut s = InputState::new(window, cx);
                s.set_value(agent.name.clone(), window, cx);
                s.placeholder("agent name")
            });
            let command_state = cx.new(|cx| {
                let mut s = InputState::new(window, cx);
                s.set_value(agent.command.clone(), window, cx);
                s.placeholder("shell command")
            });
            let id_for_name = agent.id.clone();
            let name_sub = cx.subscribe(&name_state, move |this, _, event, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.commit_agent(&id_for_name, cx);
                }
            });
            let id_for_cmd = agent.id.clone();
            let command_sub = cx.subscribe(&command_state, move |this, _, event, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.commit_agent(&id_for_cmd, cx);
                }
            });
            self.inputs.insert(
                agent.id.clone(),
                AgentInputs {
                    name: name_state,
                    command: command_state,
                    _subs: vec![name_sub, command_sub],
                },
            );
        }
    }

    fn commit_agent(&self, id: &str, cx: &mut Context<Self>) {
        let Some(input) = self.inputs.get(id) else {
            return;
        };
        let name = input.name.read(cx).value().to_string();
        let command = input.command.read(cx).value().to_string();
        self.state.read(cx).settings.update(cx, |store, cx| {
            store.update_agent(id, name, command, cx);
        });
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Re-sync inputs on every render so newly-added agents get inputs.
        self.reconcile_inputs_with_window(window, cx);

        let agents = self.state.read(cx).settings.read(cx).agents().to_vec();
        let store = self.state.read(cx).settings.clone();
        let weak = cx.entity().downgrade();

        let mut items: Vec<SettingItem> = Vec::with_capacity(agents.len() + 1);
        let len = agents.len();
        for (idx, agent) in agents.iter().enumerate() {
            let id = agent.id.clone();
            let Some(input) = self.inputs.get(&id) else {
                continue;
            };
            let name_input = input.name.clone();
            let command_input = input.command.clone();
            let is_first = idx == 0;
            let is_last = idx + 1 == len;

            let store_for_up = store.clone();
            let id_up = id.clone();
            let store_for_down = store.clone();
            let id_down = id.clone();
            let store_for_del = store.clone();
            let id_del = id.clone();

            items.push(SettingItem::render(move |_opts, _window, _cx| {
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(Label::new("Agent"))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new(("up", id_up.clone()))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::ArrowUp)
                                            .disabled(is_first)
                                            .on_click({
                                                let store = store_for_up.clone();
                                                let id = id_up.clone();
                                                move |_, _, cx| {
                                                    store.update(cx, |s, cx| {
                                                        s.move_agent(&id, MoveDir::Up, cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new(("down", id_down.clone()))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::ArrowDown)
                                            .disabled(is_last)
                                            .on_click({
                                                let store = store_for_down.clone();
                                                let id = id_down.clone();
                                                move |_, _, cx| {
                                                    store.update(cx, |s, cx| {
                                                        s.move_agent(&id, MoveDir::Down, cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new(("del", id_del.clone()))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Trash)
                                            .on_click({
                                                let store = store_for_del.clone();
                                                let id = id_del.clone();
                                                move |_, _, cx| {
                                                    store.update(cx, |s, cx| {
                                                        s.remove_agent(&id, cx);
                                                    });
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new("Name"))
                            .child(Input::new(&name_input)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new("Command"))
                            .child(Input::new(&command_input)),
                    )
                    .into_any_element()
            }));
        }

        // "+ Add agent" item.
        let store_for_add = store.clone();
        let weak_for_add = weak.clone();
        items.push(SettingItem::render(move |_opts, _window, _cx| {
            Button::new("add-agent")
                .primary()
                .label("+ Add agent")
                .on_click({
                    let store = store_for_add.clone();
                    let weak = weak_for_add.clone();
                    move |_, window, cx| {
                        store.update(cx, |s, cx| {
                            s.add_agent(cx);
                        });
                        // Force a render so the new input is created with `window`.
                        let _ = weak.update(cx, |this, cx| {
                            this.reconcile_inputs_with_window(window, cx);
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        }));

        div()
            .track_focus(&self.focus)
            .size_full()
            .child(
                Settings::new("agents-settings").pages(vec![
                    SettingPage::new("Agents").groups(vec![SettingGroup::new().items(items)]),
                ]),
            )
    }
}
```

- [ ] **Step 2: Register `settings_view` in `src/layout/mod.rs`**

Add `pub mod settings_view;` near the top of `src/layout/mod.rs`:

```rust
pub mod left_sidebar;
pub mod main_tabs;
pub mod right_sidebar;
pub mod settings_view;
```

- [ ] **Step 3: Mount the real view in the dialog**

In `src/app.rs`, find the `OpenSettings` handler from Task 6 and replace its body to instantiate and embed `SettingsView`:

```rust
            .on_action({
                let state = state.clone();
                move |_: &OpenSettings, window: &mut Window, cx: &mut App| {
                    let view =
                        cx.new(|cx| crate::layout::settings_view::SettingsView::new(state.clone(), window, cx));
                    gpui_component::Root::update(window, cx, |root, window, cx| {
                        root.open_dialog(
                            move |dialog, _window, _cx| {
                                let view = view.clone();
                                dialog
                                    .title("Settings")
                                    .content(move |content, _, _| content.child(view.clone()))
                                    .w(gpui::px(800.))
                                    .close_button(true)
                                    .overlay_closable(true)
                                    .keyboard(true)
                            },
                            window,
                            cx,
                        );
                    });
                }
            })
```

(`state` is already cloned at the top of `Render::render` in `Root`; if it isn't yet accessible at this point in the function, hoist a `let state = self.state.clone();` to the top of the render body so the closure can capture it.)

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean. If a name like `Input::new` doesn't resolve, double-check the `use gpui_component::input::{Input, InputEvent, InputState};` import — `Input` is the rendered widget, `InputState` is the entity it reads from.

- [ ] **Step 5: Manual verification**

Run: `cargo run` and exercise the full flow:
1. ⌘, opens Settings.
2. Click "+ Add agent" → a row appears with empty Name + Command inputs.
3. Type a name ("claude") and command ("claude"), tab away or press Enter → values commit.
4. Close the dialog with the X button.
5. Reopen with ⌘, → the agent is still there.
6. Quit (⌘Q) and `cargo run` again → the agent persists across restarts (file: `~/Library/Application Support/swrm/settings.json`).
7. Add a second agent, use ↑/↓ to reorder.
8. Click 🗑 on a row → row disappears.

If any step fails, debug in place — don't move on to Task 8 with a broken Settings UI.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/layout/settings_view.rs src/layout/mod.rs src/app.rs
git commit -m "feat: Implement Settings dialog with agent add/edit/remove/reorder"
```

---

## Task 8: `TabSpec`, "+" dropdown, default tab, and label disambiguation

**Files:**
- Modify: `src/layout/main_tabs.rs`

This task replaces the bare `+` button with a dropdown menu, threads a `TabSpec` enum through `TerminalTab::new`, makes `ensure_tab_for_active` pick the first agent when one exists, and uses `unique_label` so duplicate kinds get numbered.

- [ ] **Step 1: Introduce `TabSpec` and update `TerminalTab::new`**

At the top of `src/layout/main_tabs.rs`, add a new public enum (after the existing `use` block):

```rust
use swrm::settings::Agent;

pub enum TabSpec {
    Shell,
    Agent(Agent),
}
```

Then modify `TerminalTab::new` to take `spec: &TabSpec`:

```rust
impl TerminalTab {
    pub fn new(
        label: String,
        cwd: PathBuf,
        spec: &TabSpec,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let mut terminal = match spec {
            TabSpec::Shell => Terminal::spawn(&cwd, 80, 24)?,
            TabSpec::Agent(agent) => Terminal::spawn_command(&cwd, &agent.command, 80, 24)?,
        };
        // (rest of the body is unchanged: events loop, focus handle, return Self {…})
```

Keep the events-loop block and the rest of the constructor body identical — the only change is the `let mut terminal = ...` line.

- [ ] **Step 2: Update existing callers**

Search the file for `TerminalTab::new(`. The two callers are:

1. `ensure_tab_for_active` (around line 225): currently `TerminalTab::new("terminal 1".into(), cwd.clone(), cx)`. Replace with `TerminalTab::new(label, cwd.clone(), &spec, cx)` — we'll compute `label` and `spec` next.

2. `new_tab` (around line 237): currently `TerminalTab::new(label, cwd, cx)`. Replace with `TerminalTab::new(label, cwd, &TabSpec::Shell, cx)` for now — we'll replace with the dropdown later in this task.

The file won't compile yet (we still need `label` and `spec` in `ensure_tab_for_active`); that's fine, we fix it in Step 3.

- [ ] **Step 3: Derive `Clone` on `TabSpec`**

Update the enum from Step 1 to derive `Clone` — needed because we hand it to closures (dropdown items, spawn_tab) that may run more than once:

```rust
#[derive(Clone)]
pub enum TabSpec {
    Shell,
    Agent(Agent),
}
```

`Agent` already derives `Clone` (from `src/settings/mod.rs`), so this just works.

- [ ] **Step 4: Replace `ensure_tab_for_active` + `new_tab`, add `new_tab_with` + `spawn_tab`**

Replace the two existing methods (around lines 213–241) with these four methods. The new `spawn_tab` helper consolidates label disambiguation + entity construction; `new_tab_with` is the public entry point used by the dropdown:

```rust
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
        let (spec, base) = match self
            .state
            .read(cx)
            .settings
            .read(cx)
            .agents()
            .first()
            .cloned()
        {
            Some(agent) => {
                let base = agent.name.clone();
                (TabSpec::Agent(agent), base)
            }
            None => (TabSpec::Shell, "terminal".to_string()),
        };
        self.spawn_tab(&cwd, spec, &base, cx);
    }

    fn new_tab_with(&mut self, spec: TabSpec, cx: &mut Context<Self>) {
        let Some(cwd) = self.state.read(cx).active_workspace.clone() else {
            return;
        };
        let base = match &spec {
            TabSpec::Shell => "terminal".to_string(),
            TabSpec::Agent(agent) => agent.name.clone(),
        };
        self.spawn_tab(&cwd, spec, &base, cx);
    }

    fn spawn_tab(
        &mut self,
        cwd: &PathBuf,
        spec: TabSpec,
        base: &str,
        cx: &mut Context<Self>,
    ) {
        let entry = self.by_workspace.entry(cwd.clone()).or_default();
        let existing: Vec<&str> = entry
            .tabs
            .iter()
            .map(|t| t.read(cx).label.as_str())
            .collect();
        let label = swrm::tab_labels::unique_label(&existing, base);
        let cwd_clone = cwd.clone();
        // Mirrors the existing `.unwrap()` in `new_tab`: tab-spawn failure is
        // a programming error (PTY config), not a runtime expectation.
        let tab = cx.new(|cx| TerminalTab::new(label, cwd_clone, &spec, cx).unwrap());
        entry.tabs.push(tab);
        entry.active_index = entry.tabs.len() - 1;
        cx.notify();
    }
```

- [ ] **Step 5: Replace the "+" `on_click` with `dropdown_menu`**

In `MainTabsPanel::render` (around line 334), find the block that adds the "+" button:

```rust
        if has_workspace {
            bar = bar.suffix(
                Button::new("new-tab")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, _, cx| this.new_tab(cx))),
            );
        }
```

Replace with:

```rust
        if has_workspace {
            let state = self.state.clone();
            let weak = cx.entity().downgrade();
            bar = bar.suffix(
                Button::new("new-tab").ghost().small().icon(IconName::Plus).dropdown_menu(
                    move |menu, _window, cx| {
                        let agents = state.read(cx).settings.read(cx).agents().to_vec();
                        let mut menu = menu.min_w(gpui::px(180.));
                        for agent in &agents {
                            let agent = agent.clone();
                            let weak = weak.clone();
                            menu = menu.item(
                                gpui_component::menu::PopupMenuItem::new(agent.name.clone())
                                    .on_click(move |_, _, cx| {
                                        let agent = agent.clone();
                                        let _ = weak.update(cx, |this, cx| {
                                            this.new_tab_with(TabSpec::Agent(agent), cx);
                                        });
                                    }),
                            );
                        }
                        if !agents.is_empty() {
                            menu = menu.separator();
                        }
                        let weak = weak.clone();
                        menu.item(
                            gpui_component::menu::PopupMenuItem::new("Open terminal").on_click(
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.new_tab_with(TabSpec::Shell, cx);
                                    });
                                },
                            ),
                        )
                    },
                ),
            );
        }
```

Drop the old `fn new_tab` and `cmd_new_tab` private methods if they only delegated to `new_tab`. Keep `cmd_new_tab` but route it through `new_tab_with(TabSpec::Shell, cx)` so that ⌘T still works. Replace its body:

```rust
    pub fn cmd_new_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.new_tab_with(TabSpec::Shell, cx);
    }
```

If you choose: also make ⌘T spawn the first agent when one exists (so it matches the default-tab behavior). That's a small UX win. Wire it up like this instead:

```rust
    pub fn cmd_new_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let spec = self
            .state
            .read(cx)
            .settings
            .read(cx)
            .agents()
            .first()
            .cloned()
            .map(TabSpec::Agent)
            .unwrap_or(TabSpec::Shell);
        self.new_tab_with(spec, cx);
    }
```

Pick whichever — both are sensible. Recommend the second.

- [ ] **Step 6: Drop the now-unused `new_tab` private method**

Remove the original `fn new_tab(&mut self, cx: &mut Context<Self>)` (around line 231) — `new_tab_with` and `spawn_tab` replace it.

- [ ] **Step 7: Run unit + integration tests**

Run: `cargo test`
Expected: all previous tests still pass. (We didn't add new tests in Task 8 since `unique_label` was tested in Task 4 and the spawn path in Task 3; the rest is UI integration.)

- [ ] **Step 8: Build and manual smoke**

Run: `cargo run`. Verify:
1. With no agents configured, "+" still produces a dropdown with just "Open terminal".
2. Open Settings, add two agents ("claude" and "codex" with placeholder commands like `printf 'hi from %s\n' claude; cat`), close.
3. "+" dropdown now shows "claude", "codex", separator, "Open terminal".
4. Click "claude" → a tab labeled `claude` opens running the command.
5. Click "claude" again → second tab labeled `claude 2`.
6. Click "Open terminal" → tab labeled `terminal`.
7. Open a brand-new workspace via the existing "Open workspace…" flow → first tab spawns the first agent (`claude` label) automatically.
8. Remove the `claude` agent in Settings → the open `claude` and `claude 2` tabs keep running (decision from spec); "+" dropdown no longer offers `claude`.
9. ⌘W closes the active tab; ⌘T opens a new one of the first agent (or terminal if none).

- [ ] **Step 9: Commit**

```bash
cargo fmt
git add src/layout/main_tabs.rs
git commit -m "feat: Open new tabs via agent dropdown; default to first agent per workspace"
```

---

## Wrap-up

After Task 8 is committed, the feature is end-to-end functional. Run the full suite one last time:

```bash
cargo fmt --check
cargo test
cargo run   # eyeball-test the flows from Task 8 Step 7
```

If anything is rough — visual polish on the agent rows, keyboard nav inside the modal, dropdown sort order, etc. — open follow-up tasks rather than slipping them into this plan.
