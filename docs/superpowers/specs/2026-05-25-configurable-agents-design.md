# Configurable Agents — Design

**Status:** Approved 2026-05-25
**Owner:** vandermeijs@redkiwi.nl

## Goal

Let users define named "agents" (a label + a shell command) in a Settings UI, pick one when opening a new tab, and have the first agent open automatically as the default tab for any new workspace. An agent is just a configurable command the terminal is launched with — `claude`, `codex`, `aider`, a project-specific REPL, anything.

## Non-goals

- Per-workspace agent overrides
- Env-var / arg config that the user can't already express in their command string
- File-watching `settings.json` for live reload
- Killing/restarting tabs when their source agent is edited or deleted
- Reorder by drag (use up/down arrows for now)
- Validation that the agent's command exists on `$PATH` — let the shell surface the error

## Architecture

A new `settings` module mirrors the existing `workspace` module. A `SettingsStore` entity owns the in-memory `AppSettings`, auto-saves to disk on every mutation, and emits a typed change event. `AppState` gains a handle to it alongside `WorkspaceStore`. UI consumers read agents through the store; mutations go through dedicated methods on the store.

A `TabSpec` enum threads through the tab-spawn boundary: `Shell` (existing behavior, runs `$SHELL`) or `Agent(Agent)` (runs `$SHELL -c <command>` via a new `Terminal::spawn_command`). Open tabs hold no live reference to their originating agent — edits and deletes in Settings only affect *new* tabs from the dropdown.

The "+" button's dropdown is rebuilt from `SettingsStore::agents()` on each render of `MainTabsPanel`, so config changes propagate without explicit invalidation.

Settings opens via a native macOS menu entry (⌘,) and renders inside a gpui-component `Modal` overlaid on the main window. The application menu also surfaces existing keybind-only actions (New Tab, Close Tab, Toggle sidebars) so they're discoverable.

## Data model

```rust
// src/settings/mod.rs

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Agent {
    /// Stable id generated on add. Used as the SettingItem key so Name/Command
    /// `InputState`s stay glued to the right row across reorders and deletes.
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AppSettings {
    #[serde(default)]
    pub agents: Vec<Agent>,
}
```

Every field on `AppSettings` carries `#[serde(default)]` so future additions (theme, font size, …) load cleanly against older `settings.json` files.

### ID generation

On `add_agent`: scan existing ids, parse the trailing integer of any matching `agent-(\d+)`, take `max + 1`. Format `format!("agent-{n}")`. Stable across saves, collision-free, and cheap because the file is tiny.

## Persistence

File: `~/Library/Application Support/swrm/settings.json` via `dirs::config_dir()`. Module mirrors `workspace::persistence`:

```rust
// src/settings/persistence.rs
pub fn config_path() -> Result<PathBuf>;
pub fn load_from(path: &Path) -> Result<AppSettings>;
pub fn save_to(path: &Path, settings: &AppSettings) -> Result<()>;
pub fn load() -> Result<AppSettings>;
pub fn save(settings: &AppSettings) -> Result<()>;
```

Sample on-disk shape:

```json
{
  "agents": [
    { "id": "agent-1", "name": "claude", "command": "claude" },
    { "id": "agent-2", "name": "codex",  "command": "codex --print" }
  ]
}
```

### Error behavior

- Missing file → `AppSettings::default()` (empty agents).
- Malformed JSON → `tracing::warn!` and fall back to `AppSettings::default()`. Don't overwrite the bad file; let the user fix it via the UI.
- Save failure → `tracing::warn!`. In-memory state is still correct; the next mutation triggers another save attempt.

No file watcher. No concurrent-edit handling. Same posture as `WorkspaceStore`.

## Store

```rust
// src/settings/store.rs

pub struct SettingsStore { pub settings: AppSettings }

#[derive(Clone, Debug)]
pub enum SettingsEvent { Changed }

impl SettingsStore {
    pub fn new(cx: &mut Context<Self>) -> Self;           // calls persistence::load()
    pub fn agents(&self) -> &[Agent];

    pub fn add_agent(&mut self, cx: &mut Context<Self>);                                     // appends a blank agent
    pub fn update_agent(&mut self, id: &str, name: String, command: String, cx: &mut Context<Self>);
    pub fn remove_agent(&mut self, id: &str, cx: &mut Context<Self>);
    pub fn move_agent(&mut self, id: &str, direction: MoveDir, cx: &mut Context<Self>);     // Up or Down
}
```

Every mutator calls `persistence::save(&self.settings)` and `cx.emit(SettingsEvent::Changed)`.

## Application menu

Set at startup in `main.rs` via `cx.set_menus(...)`:

| Menu | Items |
| --- | --- |
| **swrm** | About swrm, ⎯, Settings… (⌘,), ⎯, Quit swrm (⌘Q) |
| **File** | New Tab (⌘T), Close Tab (⌘W) |
| **View** | Toggle Left Sidebar (⌘B), Toggle Right Sidebar (⌘L) |

New actions: `OpenSettings` (⌘,), `About` (no shortcut), `Quit` (⌘Q) added to `app::actions!`. The rest reuse existing actions (`NewTab`, `CloseTab`, `ToggleLeftSidebar`, `ToggleRightSidebar`). OS-provided items like Hide / Show All / Window-menu defaults are out of scope for this change — we'll add them when we want them.

`OpenSettings` is handled in `app::Root` by flipping `AppState::settings_open = true`; re-render shows the modal.

## Settings UI

Hosted in a gpui-component `Modal` overlaid via `Root::render_dialog_layer`. Closable by ✕, Escape, or click-outside.

Layout uses gpui-component's `Settings` → one `SettingPage::new("Agents")`. The page body renders a custom flex column:

```
┌─ Settings ────────────────────────────────── ✕ ─┐
│ ┌──────────┐ ┌─────────────────────────────────┐│
│ │ Agents   │ │ Agents                          ││
│ │          │ │                                 ││
│ │          │ │ ┌─ Agent ─────────── ↑ ↓ 🗑 ─┐ ││
│ │          │ │ │ Name    [ claude         ]  │ ││
│ │          │ │ │ Command [ claude         ]  │ ││
│ │          │ │ └─────────────────────────────┘ ││
│ │          │ │                                 ││
│ │          │ │ ┌─ Agent ─────────── ↑ ↓ 🗑 ─┐ ││
│ │          │ │ │ Name    [ codex          ]  │ ││
│ │          │ │ │ Command [ codex          ]  │ ││
│ │          │ │ └─────────────────────────────┘ ││
│ │          │ │                                 ││
│ │          │ │              [ + Add agent ]    ││
│ └──────────┘ └─────────────────────────────────┘│
└──────────────────────────────────────────────── ┘
```

The `SettingsView` entity owns a `HashMap<String, (InputState, InputState)>` keyed by `agent.id` — Name input and Command input per row. Input state is created lazily on first render of a row and dropped when the agent is removed. Change handlers call `SettingsStore::update_agent(id, name, command)` on commit (input loses focus or Enter). The "+ Add agent" button calls `add_agent`; ↑/↓ call `move_agent`; 🗑 calls `remove_agent`.

The sidebar shows just `Agents` for now — the framework supports more pages later without changes.

## "+" dropdown

In `layout/main_tabs.rs`, the existing "+" button's `.on_click` is replaced with `.dropdown_menu(|menu, _, _| { ... })`. The closure reads `SettingsStore::agents()` and builds:

```
┌─────────────────┐
│ claude          │   ← one item per agent (in store order)
│ codex           │
│ ─────────────── │   ← separator (omitted when there are 0 agents)
│ Open terminal   │
└─────────────────┘
```

Each agent item's `on_click` calls `MainTabsPanel::new_tab_with(TabSpec::Agent(agent.clone()))`; the terminal item calls `new_tab_with(TabSpec::Shell)`. With zero agents, the menu has just `Open terminal` and no separator.

## Tab spawning

```rust
// src/terminal/mod.rs

impl Terminal {
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<Self>;                              // unchanged
    pub fn spawn_command(cwd: &Path, command: &str, cols: u16, rows: u16) -> Result<Self>;       // NEW
}
```

`spawn_command` mirrors `spawn` but sets `tty_opts.shell = Some(Shell::new($SHELL, vec!["-c".into(), command.into()]))`. The `child_env()` override from the title-leak fix continues to apply unchanged.

```rust
// src/layout/main_tabs.rs (or a sibling module)

pub enum TabSpec {
    Shell,
    Agent(Agent),
}
```

`TerminalTab::new(label, cwd, spec, cx)` matches on `spec` and calls the right `Terminal::spawn*`. The tab struct doesn't store the spec — once launched, it's just a labeled PTY.

## Default-tab logic

In `MainTabsPanel::ensure_tab_for_active`:

```text
if workspace has no tabs:
    let (spec, base) = match settings.agents.first() {
        Some(agent) => (TabSpec::Agent(agent.clone()), agent.name.clone()),
        None        => (TabSpec::Shell,                "terminal".to_string()),
    };
    spawn(label = unique_label(workspace, &base), spec)
```

`new_tab_with` uses the same `unique_label` helper but takes the base name from the spec (`agent.name` for `Agent`, `"terminal"` for `Shell`).

## Tab label disambiguation

```rust
// src/layout/tab_labels.rs (extracted so tests can hit it without the UI)
pub fn unique_label(existing: &[&str], base: &str) -> String;
```

Algorithm: if `base` isn't in `existing`, return `base`. Otherwise return `format!("{base} {n}")` for the smallest `n ≥ 2` such that `"{base} {n}"` isn't in `existing`. Gap-fills after deletes:

| `existing` | `base` | result |
| --- | --- | --- |
| `[]` | `claude` | `claude` |
| `["claude"]` | `claude` | `claude 2` |
| `["claude", "claude 2", "claude 3"]` | `claude` | `claude 4` |
| `["claude", "claude 3"]` | `claude` | `claude 2` |
| `["terminal", "claude"]` | `terminal` | `terminal 2` |

## Testing

Pattern follows the existing `tests/` layout — integration tests per concern, no `#[test]` blocks inside `src/`.

### `tests/settings_persistence.rs`

- Round-trip a populated `AppSettings` through `save_to` → `load_from` against a tempfile; assert equal.
- `load_from` on a non-existent path → `AppSettings::default()`.
- `load_from` on malformed JSON → `AppSettings::default()` (we don't assert on the warning).
- Backwards-compat: a JSON blob missing the `agents` key loads as default. Locks in `#[serde(default)]`.

### `tests/agent_command_spawn.rs`

End-to-end PTY spawn for `TabSpec::Agent`:

- Spawn `Terminal::spawn_command(tempdir, "printf 'AGENT=[%s]' yes", 80, 6)`.
- Poll the snapshot until the grid contains `AGENT=[yes]` or a 5s timeout fires (same poll pattern as `child_shell_sees_xterm_256color_term`).
- Asserts: `$SHELL -c` is wired correctly, `child_env()` still applies, the child writes to the PTY.

### `tests/tab_labels.rs`

Pure-logic test of `unique_label` for each row of the table above.

### Manual verification

- Open Settings via ⌘, → modal appears.
- Add an agent → close modal → quit/relaunch app → agent still there.
- Click "+" → dropdown shows agents + separator + "Open terminal".
- Open a brand-new workspace (Open workspace… or `git worktree add` via the existing flow) → first tab spawns the first agent automatically with the agent's name as the label.
- Two `claude` tabs in one workspace → labels are `claude` and `claude 2`.
- Remove an agent that has open tabs → tabs keep running, dropdown no longer offers that agent.
- Hand-edit `settings.json` to malformed JSON → app starts with empty agents, no crash, warning in stderr.

### Existing tests stay green

`workspace_persistence`, `git_status`, `vt_smoke` (including `child_shell_sees_xterm_256color_term` and `screen_title_sequence_leaks_into_grid` from the title-leak fix). The new env override in `child_env()` is unaffected.

## File touch list

**New files**

- `src/settings/mod.rs` — `Agent`, `AppSettings`, re-exports
- `src/settings/persistence.rs` — load/save
- `src/settings/store.rs` — `SettingsStore` + `SettingsEvent`
- `src/layout/settings_view.rs` — modal Settings UI
- `src/layout/tab_labels.rs` — `unique_label` helper
- `tests/settings_persistence.rs`
- `tests/agent_command_spawn.rs`
- `tests/tab_labels.rs`

**Modified files**

- `src/lib.rs` — `pub mod settings;`
- `src/main.rs` — call `cx.set_menus(...)`, bind `cmd-,` to `OpenSettings`, register the modal layer
- `src/app.rs` — new `OpenSettings` action, handle it on `Root`
- `src/app_state.rs` — add `settings: Entity<SettingsStore>`, `settings_open: bool`, toggle helper
- `src/layout/mod.rs` — wire `SettingsStore` into `Layout::build`
- `src/layout/main_tabs.rs` — `TabSpec`, `.dropdown_menu` on "+", `new_tab_with`, updated `ensure_tab_for_active`, use `unique_label`
- `src/terminal/mod.rs` — `Terminal::spawn_command`
- `src/terminal/backend.rs` — `Backend::spawn_command` (mirrors `spawn`, sets `Shell::new($SHELL, vec!["-c", cmd])`)
- `Cargo.toml` — no new deps expected; `uuid` not needed (we use `agent-{n}` ids)
