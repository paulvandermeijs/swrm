# Agent Status Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Track the lifecycle status of each agent terminal (idle/working/notify/done) and surface a colored indicator next to the matching workspace in the left sidebar.

**Architecture:** swrm starts a tiny localhost HTTP server (TCP port `0`, kernel-assigned) at app init. Each agent terminal tab gets a unique `tab_id`, and when the agent command contains the literal placeholder `$CLAUDE_SETTINGS` (or `${CLAUDE_SETTINGS}`), swrm writes a per-tab Claude Code settings JSON to a temp file and string-substitutes the placeholder with the file's absolute path before launching the PTY. The settings file's hooks `curl`-POST to `http://127.0.0.1:<port>/event/<tab_id>/<event_name>`; the server thread forwards each request through an `mpsc` channel to a gpui async task that updates an `AgentStatusStore` entity. The left sidebar observes the store and renders a colored dot beside any workspace that owns at least one tab with an active status.

**Tech Stack:** Rust, gpui + gpui-component (existing), `std::net::TcpListener` (hand-rolled HTTP — no new HTTP dep), `serde_json` (already a dep), `futures::channel::mpsc` (already used for terminal events). External runtime requirement: `curl` on PATH (ships with macOS).

---

## File structure

New files:
- `src/agent_status/mod.rs` — module re-exports (`AgentStatus`, `AgentStatusStore`, `HookEvent`, `start_server`, `build_claude_settings_json`, `substitute_placeholder`).
- `src/agent_status/event.rs` — `AgentStatus` enum + priority ordering.
- `src/agent_status/store.rs` — `AgentStatusStore` gpui entity. Maps `tab_id → (workspace_path, AgentStatus)`. Provides `status_for_workspace(path)`.
- `src/agent_status/server.rs` — TCP listener on a background thread + a pure `parse_event_path` helper. Emits `HookEvent { tab_id, event }` through an `UnboundedSender`.
- `src/agent_status/settings_file.rs` — pure builder for Claude Code's hooks JSON; pure `substitute_placeholder(command, path) -> String`; pure `temp_settings_path(tab_id) -> PathBuf`.
- `tests/agent_status.rs` — integration tests for the pure helpers and a server round-trip.

Modified files:
- `src/lib.rs` — register `pub mod agent_status;` after `app_state`.
- `src/app_state.rs` — add `agent_status: Entity<AgentStatusStore>` to `AppState`; start the server at `AppState::new`; spawn the channel-pump task.
- `src/layout/main_tabs.rs` — generate `tab_id` per agent tab; on spawn, write the temp settings file and substitute the placeholder; register in `AgentStatusStore`; unregister on close.
- `src/layout/left_sidebar.rs` — subscribe to `AgentStatusStore`; render a colored dot beside each workspace whose aggregate status is not `None`.
- `src/layout/settings_view.rs` — add a single-line hint under the command input: "Use `$CLAUDE_SETTINGS` in the command to enable status tracking".

Public API stays at the top of each file; private items at the bottom — same convention the rest of the repo uses.

---

### Task 1: `AgentStatus` enum

**Files:**
- Create: `src/agent_status/mod.rs`
- Create: `src/agent_status/event.rs`
- Create: `tests/agent_status.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/agent_status.rs` (create it):

```rust
use swrm::agent_status::AgentStatus;

#[test]
fn agent_status_priority_orders_attention_first() {
    // Higher priority = more urgent. Notify is the most attention-worthy.
    assert!(AgentStatus::Notify.priority() > AgentStatus::Done.priority());
    assert!(AgentStatus::Done.priority() > AgentStatus::Working.priority());
    assert!(AgentStatus::Working.priority() > AgentStatus::Idle.priority());
}

#[test]
fn agent_status_from_str_round_trips_known() {
    for &(s, expected) in &[
        ("notify", AgentStatus::Notify),
        ("done", AgentStatus::Done),
        ("working", AgentStatus::Working),
        ("idle", AgentStatus::Idle),
    ] {
        assert_eq!(AgentStatus::from_wire(s), Some(expected));
    }
}

#[test]
fn agent_status_from_str_unknown_is_none() {
    assert_eq!(AgentStatus::from_wire(""), None);
    assert_eq!(AgentStatus::from_wire("bogus"), None);
    assert_eq!(AgentStatus::from_wire("NOTIFY"), None); // case-sensitive
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test agent_status agent_status_ -- --nocapture`
Expected: FAIL — `agent_status` module not found.

- [ ] **Step 3: Create the module skeleton**

Create `src/agent_status/mod.rs`:

```rust
pub mod event;

pub use event::AgentStatus;
```

Create `src/agent_status/event.rs`:

```rust
/// Lifecycle status reported by an agent's hooks. Wire strings match
/// agent-status / Claude Code conventions (`notify`, `done`, `working`,
/// `idle`) so they can be reused by other agents later. Unknown wire
/// values are dropped at the parsing boundary (`from_wire` returns
/// `None`) rather than carried through as an `Unknown` variant — the
/// current set is sufficient for the indicator and a future agent that
/// needs a new value should add a variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    Notify,
    Done,
    Working,
    Idle,
}

impl AgentStatus {
    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "notify" => Some(Self::Notify),
            "done" => Some(Self::Done),
            "working" => Some(Self::Working),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }

    /// Higher = more attention-worthy. Used to fold multiple tabs'
    /// statuses into a single workspace-level indicator.
    pub fn priority(self) -> u8 {
        match self {
            Self::Notify => 4,
            Self::Done => 3,
            Self::Working => 2,
            Self::Idle => 1,
        }
    }
}
```

- [ ] **Step 4: Register the module**

Edit `src/lib.rs`. Find the `pub mod` block and add `pub mod agent_status;` alphabetically (between `app_state` and `git`):

```rust
pub mod agent_status;
pub mod app_state;
pub mod git;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test agent_status agent_status_`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/agent_status/mod.rs src/agent_status/event.rs src/lib.rs tests/agent_status.rs
git commit -m "$(cat <<'EOF'
feat: add AgentStatus enum with wire parsing and priority ordering
EOF
)"
```

---

### Task 2: Settings JSON builder + placeholder substitution

**Files:**
- Create: `src/agent_status/settings_file.rs`
- Modify: `src/agent_status/mod.rs`
- Modify: `tests/agent_status.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests/agent_status.rs`:

```rust
use swrm::agent_status::{build_claude_settings_json, substitute_placeholder};

#[test]
fn claude_settings_json_wires_all_status_hooks() {
    let json = build_claude_settings_json("http://127.0.0.1:51234", "tab-abc");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Every status-bearing event has a curl POST to the right URL.
    let expectations = [
        ("PermissionRequest", "notify"),
        ("Stop", "done"),
        ("UserPromptSubmit", "working"),
        ("PreToolUse", "working"),
        ("PostToolUse", "working"),
        ("SessionStart", "idle"),
    ];
    for (event, status) in expectations {
        let cmd = parsed
            .pointer(&format!("/hooks/{event}/0/hooks/0/command"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("missing {event} hook"));
        let expected_url = format!("http://127.0.0.1:51234/event/tab-abc/{status}");
        assert!(
            cmd.contains(&expected_url),
            "{event}: expected URL {expected_url} in cmd: {cmd}",
        );
        assert!(cmd.contains("curl"), "{event}: cmd should curl: {cmd}");
    }
}

#[test]
fn claude_settings_json_does_not_subscribe_to_notification() {
    // Claude Code's Notification hook fires on a timer for idle_prompt,
    // which would spuriously flip a freshly-cleared session back to notify.
    // PermissionRequest covers the legitimate permission case; matches
    // agent-status's reasoning.
    let json = build_claude_settings_json("http://127.0.0.1:1", "x");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.pointer("/hooks/Notification").is_none());
}

#[test]
fn substitute_placeholder_replaces_bare_form() {
    let out = substitute_placeholder("claude --settings $CLAUDE_SETTINGS", "/tmp/x.json");
    assert_eq!(out, "claude --settings /tmp/x.json");
}

#[test]
fn substitute_placeholder_replaces_braced_form() {
    let out = substitute_placeholder("claude --settings ${CLAUDE_SETTINGS}", "/tmp/x.json");
    assert_eq!(out, "claude --settings /tmp/x.json");
}

#[test]
fn substitute_placeholder_leaves_command_alone_when_absent() {
    let out = substitute_placeholder("claude", "/tmp/x.json");
    assert_eq!(out, "claude");
}

#[test]
fn substitute_placeholder_handles_multiple_occurrences() {
    let out = substitute_placeholder("a $CLAUDE_SETTINGS b ${CLAUDE_SETTINGS} c", "/p");
    assert_eq!(out, "a /p b /p c");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test agent_status`
Expected: FAIL — `build_claude_settings_json` / `substitute_placeholder` not found.

- [ ] **Step 3: Create the builder module**

Create `src/agent_status/settings_file.rs`:

```rust
use serde_json::json;

/// Build the Claude Code `--settings` file contents pointing at the swrm
/// hook server. `base_url` is the swrm server origin (`http://127.0.0.1:PORT`,
/// no trailing slash); `tab_id` is opaque to the hook and identifies the tab
/// in `AgentStatusStore`.
///
/// Mirrors agent-status's wiring with one key omission: no `Notification`
/// hook (see `build_extension_claude_code_does_not_subscribe_to_notification`
/// in agent-status — it flips freshly-cleared sessions back to `notify` on
/// the idle_prompt timer). `SessionEnd` is also omitted because swrm
/// unregisters the tab on close; an extra `clear` hook would just duplicate
/// that signal.
pub fn build_claude_settings_json(base_url: &str, tab_id: &str) -> String {
    let url = |status: &str| format!("{base_url}/event/{tab_id}/{status}");
    let cmd = |status: &str| {
        // --max-time 1: a slow hook must not block Claude. -s: quiet.
        // -o /dev/null: swallow the empty body. -X POST: no body needed.
        format!(
            "curl -s --max-time 1 -o /dev/null -X POST {}",
            url(status)
        )
    };

    let value = json!({
        "hooks": {
            "PermissionRequest": [{"hooks": [{"type": "command", "command": cmd("notify")}]}],
            "Stop":              [{"hooks": [{"type": "command", "command": cmd("done")}]}],
            "UserPromptSubmit":  [{"hooks": [{"type": "command", "command": cmd("working")}]}],
            "PreToolUse":        [{"hooks": [{"type": "command", "command": cmd("working")}]}],
            "PostToolUse":       [{"hooks": [{"type": "command", "command": cmd("working")}]}],
            "SessionStart":      [{"hooks": [{"type": "command", "command": cmd("idle")}]}],
        }
    });
    serde_json::to_string_pretty(&value).expect("serde_json::Value always serializes")
}

/// Replace every occurrence of `$CLAUDE_SETTINGS` and `${CLAUDE_SETTINGS}`
/// in `command` with `path`. Bare and braced forms both work; the command
/// is passed to `$SHELL -c` so we substitute on the swrm side rather than
/// relying on the spawned shell's variable expansion.
pub fn substitute_placeholder(command: &str, path: &str) -> String {
    command
        .replace("${CLAUDE_SETTINGS}", path)
        .replace("$CLAUDE_SETTINGS", path)
}
```

- [ ] **Step 4: Re-export from mod.rs**

Edit `src/agent_status/mod.rs`:

```rust
pub mod event;
pub mod settings_file;

pub use event::AgentStatus;
pub use settings_file::{build_claude_settings_json, substitute_placeholder};
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test agent_status`
Expected: PASS (all tests so far).

- [ ] **Step 6: Commit**

```bash
git add src/agent_status/mod.rs src/agent_status/settings_file.rs tests/agent_status.rs
git commit -m "$(cat <<'EOF'
feat: build Claude Code settings JSON and substitute the placeholder
EOF
)"
```

---

### Task 3: Hook server (TCP + path parsing)

**Files:**
- Create: `src/agent_status/server.rs`
- Modify: `src/agent_status/mod.rs`
- Modify: `tests/agent_status.rs`

- [ ] **Step 1: Write the failing parsing tests**

Append to `tests/agent_status.rs`:

```rust
use swrm::agent_status::server::parse_event_path;

#[test]
fn parse_event_path_extracts_tab_and_event() {
    assert_eq!(
        parse_event_path("/event/tab-abc/notify"),
        Some(("tab-abc", "notify")),
    );
}

#[test]
fn parse_event_path_rejects_wrong_prefix() {
    assert_eq!(parse_event_path("/other/x/y"), None);
    assert_eq!(parse_event_path("event/x/y"), None);
}

#[test]
fn parse_event_path_rejects_missing_event_segment() {
    assert_eq!(parse_event_path("/event/tab-abc"), None);
    assert_eq!(parse_event_path("/event/tab-abc/"), None);
}

#[test]
fn parse_event_path_rejects_extra_segments() {
    // Three segments after /event/ would let a hook write to a nested path
    // we don't recognise — reject explicitly.
    assert_eq!(parse_event_path("/event/tab-abc/notify/extra"), None);
}

#[test]
fn parse_event_path_strips_query_string() {
    assert_eq!(
        parse_event_path("/event/tab-abc/notify?foo=bar"),
        Some(("tab-abc", "notify")),
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test agent_status parse_event_path`
Expected: FAIL — `server` module not found.

- [ ] **Step 3: Implement the server module**

Create `src/agent_status/server.rs`:

```rust
use anyhow::{Context, Result};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// One hook delivery from a tab's settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEvent {
    pub tab_id: String,
    pub event: String,
}

/// Bind a TCP listener on `127.0.0.1:0` (kernel-assigned port), spawn a
/// background thread that accepts hook POSTs, and return the bound port
/// plus a receiver of parsed `HookEvent`s. The thread runs for the
/// lifetime of the process — there is no explicit shutdown signal, and
/// the OS reclaims the socket on exit.
pub fn start_server() -> Result<(u16, UnboundedReceiver<HookEvent>)> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind agent-status server")?;
    let port = listener
        .local_addr()
        .context("agent-status server local_addr")?
        .port();
    let (tx, rx) = unbounded::<HookEvent>();
    std::thread::Builder::new()
        .name("swrm-agent-status-server".into())
        .spawn(move || run_accept_loop(listener, tx))
        .context("spawn agent-status server thread")?;
    Ok((port, rx))
}

/// Extract `(tab_id, event)` from a request path like `/event/<tab_id>/<event>`.
/// Returns `None` for any other shape (wrong prefix, missing segment, extra
/// segments). Query strings (`?…`) are stripped before parsing.
pub fn parse_event_path(path: &str) -> Option<(&str, &str)> {
    let path = path.split('?').next().unwrap_or(path);
    let rest = path.strip_prefix("/event/")?;
    // Exactly two non-empty segments.
    let mut parts = rest.split('/');
    let tab_id = parts.next()?;
    let event = parts.next()?;
    if tab_id.is_empty() || event.is_empty() {
        return None;
    }
    if parts.next().is_some() {
        return None;
    }
    Some((tab_id, event))
}

fn run_accept_loop(listener: TcpListener, tx: UnboundedSender<HookEvent>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        // Hook handlers run synchronously and complete in microseconds; no
        // worker pool needed. If a future agent fires many concurrent hooks
        // we can revisit.
        if let Err(err) = handle_one(stream, &tx) {
            tracing::debug!(?err, "hook request handling failed");
        }
    }
}

fn handle_one(mut stream: TcpStream, tx: &UnboundedSender<HookEvent>) -> Result<()> {
    // We only need the request line. Cap at 1 KiB to bound memory.
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).context("read hook request")?;
    let req = &buf[..n];
    let line_end = req
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(req.len());
    let line = std::str::from_utf8(&req[..line_end]).unwrap_or("");
    // "POST /event/<tab>/<event> HTTP/1.1"
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    if method == "POST" {
        if let Some((tab_id, event)) = parse_event_path(path) {
            let _ = tx.unbounded_send(HookEvent {
                tab_id: tab_id.to_string(),
                event: event.to_string(),
            });
        }
    }
    // Reply 204 unconditionally — curl exits 0 whether we recognised the
    // request or not, which is fine because hook scripts ignore the response.
    let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    Ok(())
}
```

- [ ] **Step 4: Re-export from mod.rs**

Edit `src/agent_status/mod.rs`:

```rust
pub mod event;
pub mod server;
pub mod settings_file;

pub use event::AgentStatus;
pub use server::{HookEvent, start_server};
pub use settings_file::{build_claude_settings_json, substitute_placeholder};
```

- [ ] **Step 5: Add a server round-trip integration test**

Append to `tests/agent_status.rs`:

```rust
use futures::StreamExt;
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;
use swrm::agent_status::{HookEvent, start_server};

#[test]
fn server_receives_post_and_dispatches_hook_event() {
    let (port, mut rx) = start_server().expect("start server");

    // Open the TCP socket directly so the test doesn't depend on curl.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .write_all(
            b"POST /event/tab-xyz/notify HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        )
        .unwrap();
    drop(stream);

    // Pump the futures channel until we get our event or time out.
    let runtime = futures::executor::block_on(async {
        futures::future::select(
            Box::pin(rx.next()),
            Box::pin(async {
                futures_timer::Delay::new(Duration::from_secs(2)).await;
                Option::<HookEvent>::None
            }),
        )
        .await
    });
    let event = match runtime {
        futures::future::Either::Left((Some(e), _)) => e,
        _ => panic!("did not receive hook event"),
    };
    assert_eq!(event.tab_id, "tab-xyz");
    assert_eq!(event.event, "notify");
}
```

Add the test-only timer dep. Edit `Cargo.toml`'s `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
futures-timer = "3"
```

- [ ] **Step 6: Run tests**

Run: `cargo test --test agent_status`
Expected: PASS (parse + round-trip).

- [ ] **Step 7: Commit**

```bash
git add src/agent_status/mod.rs src/agent_status/server.rs tests/agent_status.rs Cargo.toml Cargo.lock
git commit -m "$(cat <<'EOF'
feat: localhost hook server for agent status events
EOF
)"
```

---

### Task 4: `AgentStatusStore` entity

**Files:**
- Create: `src/agent_status/store.rs`
- Modify: `src/agent_status/mod.rs`
- Modify: `tests/agent_status.rs`

- [ ] **Step 1: Write the failing aggregation tests**

Append to `tests/agent_status.rs`:

```rust
use std::path::PathBuf;
use swrm::agent_status::store::aggregate_status;
use swrm::agent_status::AgentStatus;

#[test]
fn aggregate_status_returns_none_when_empty() {
    let entries: Vec<(PathBuf, AgentStatus)> = vec![];
    let target = PathBuf::from("/ws/a");
    assert_eq!(aggregate_status(&entries, &target), None);
}

#[test]
fn aggregate_status_picks_highest_priority_for_workspace() {
    let entries = vec![
        (PathBuf::from("/ws/a"), AgentStatus::Idle),
        (PathBuf::from("/ws/a"), AgentStatus::Notify),
        (PathBuf::from("/ws/a"), AgentStatus::Working),
        (PathBuf::from("/ws/b"), AgentStatus::Done),
    ];
    assert_eq!(
        aggregate_status(&entries, &PathBuf::from("/ws/a")),
        Some(AgentStatus::Notify),
    );
    assert_eq!(
        aggregate_status(&entries, &PathBuf::from("/ws/b")),
        Some(AgentStatus::Done),
    );
    assert_eq!(
        aggregate_status(&entries, &PathBuf::from("/ws/c")),
        None,
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test agent_status aggregate_status`
Expected: FAIL — `store` module not found.

- [ ] **Step 3: Implement the store**

Create `src/agent_status/store.rs`:

```rust
use super::AgentStatus;
use gpui::{Context, EventEmitter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum AgentStatusEvent {
    Changed,
}

/// Maps each registered agent tab to its workspace and most recent status.
/// One row per tab; a workspace with no agent tabs has no entries here.
pub struct AgentStatusStore {
    entries: HashMap<String, (PathBuf, AgentStatus)>,
}

impl AgentStatusStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a tab so its workspace can show an indicator. Starts at
    /// `AgentStatus::Idle` — Claude's `SessionStart` hook will overwrite
    /// it within a few hundred ms, but we set a baseline now so the
    /// indicator does not flicker on between empty / first-hook.
    pub fn register(&mut self, tab_id: String, workspace: PathBuf, cx: &mut Context<Self>) {
        self.entries
            .insert(tab_id, (workspace, AgentStatus::Idle));
        cx.emit(AgentStatusEvent::Changed);
        cx.notify();
    }

    pub fn unregister(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        if self.entries.remove(tab_id).is_some() {
            cx.emit(AgentStatusEvent::Changed);
            cx.notify();
        }
    }

    /// Apply a hook event. Unknown `tab_id`s are dropped silently —
    /// happens when a hook fires after the tab has been closed.
    pub fn set_status(&mut self, tab_id: &str, status: AgentStatus, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get_mut(tab_id) else {
            return;
        };
        if entry.1 == status {
            return;
        }
        entry.1 = status;
        cx.emit(AgentStatusEvent::Changed);
        cx.notify();
    }

    pub fn status_for_workspace(&self, workspace: &Path) -> Option<AgentStatus> {
        let entries: Vec<(PathBuf, AgentStatus)> =
            self.entries.values().map(|(p, s)| (p.clone(), *s)).collect();
        aggregate_status(&entries, workspace)
    }
}

impl EventEmitter<AgentStatusEvent> for AgentStatusStore {}

/// Pure aggregation helper, separated for testing. Picks the
/// highest-priority status among entries whose workspace matches.
pub fn aggregate_status(
    entries: &[(PathBuf, AgentStatus)],
    workspace: &Path,
) -> Option<AgentStatus> {
    entries
        .iter()
        .filter(|(p, _)| p.as_path() == workspace)
        .map(|(_, s)| *s)
        .max_by_key(|s| s.priority())
}
```

- [ ] **Step 4: Re-export from mod.rs**

Edit `src/agent_status/mod.rs`:

```rust
pub mod event;
pub mod server;
pub mod settings_file;
pub mod store;

pub use event::AgentStatus;
pub use server::{HookEvent, start_server};
pub use settings_file::{build_claude_settings_json, substitute_placeholder};
pub use store::{AgentStatusEvent, AgentStatusStore};
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test agent_status`
Expected: PASS (all tests so far).

- [ ] **Step 6: Commit**

```bash
git add src/agent_status/mod.rs src/agent_status/store.rs tests/agent_status.rs
git commit -m "$(cat <<'EOF'
feat: AgentStatusStore tracking per-tab status with workspace aggregation
EOF
)"
```

---

### Task 5: Wire `AgentStatusStore` and server into `AppState`

**Files:**
- Modify: `src/app_state.rs`

- [ ] **Step 1: Read the current AppState**

Run: `cat src/app_state.rs`

You should see `pub workspaces`, `pub settings`, etc. The new field follows the same pattern: another `Entity<…>` constructed inside `AppState::new`.

- [ ] **Step 2: Add the field and wire up the server pump**

Replace the entire `src/app_state.rs` with:

```rust
use crate::agent_status::{AgentStatus, AgentStatusStore, start_server};
use crate::settings::SettingsStore;
use crate::workspace::WorkspaceStore;
use futures::StreamExt;
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
    pub agent_status: Entity<AgentStatusStore>,
    /// Origin of the agent-status hook server (`http://127.0.0.1:<port>`).
    /// Baked into each tab's generated Claude settings file.
    pub agent_status_origin: String,
    pub active_workspace: Option<PathBuf>,
    pub left_sidebar_visible: bool,
    pub right_sidebar_visible: bool,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let workspaces = cx.new(|cx| WorkspaceStore::load(cx));
        let settings = cx.new(|cx| SettingsStore::load(cx));
        let agent_status = cx.new(|_cx| AgentStatusStore::new());

        let (port, mut rx) = match start_server() {
            Ok(pair) => pair,
            Err(err) => {
                tracing::warn!(?err, "agent-status server failed to start; tabs will spawn without status tracking");
                // Sentinel that won't match a real loopback — guarantees the
                // placeholder substitution still produces a file (so the
                // user's `claude --settings $CLAUDE_SETTINGS` invocation
                // doesn't break), it just won't deliver events anywhere.
                (0u16, futures::channel::mpsc::unbounded().1)
            }
        };
        let agent_status_origin = format!("http://127.0.0.1:{port}");

        let store_weak = agent_status.downgrade();
        cx.spawn(async move |_, cx| {
            while let Some(evt) = rx.next().await {
                let Some(status) = AgentStatus::from_wire(&evt.event) else {
                    tracing::debug!(event = %evt.event, "ignoring unknown status");
                    continue;
                };
                let _ = store_weak.update(cx, |store, cx| {
                    store.set_status(&evt.tab_id, status, cx);
                });
            }
        })
        .detach();

        Self {
            workspaces,
            settings,
            agent_status,
            agent_status_origin,
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

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build`
Expected: build succeeds. Warnings about unused imports are fine to fix inline if any.

- [ ] **Step 4: Smoke-run the app**

Run: `cargo run` in a separate terminal, click around briefly to ensure the window comes up. Close it.
Expected: no panics, window renders. (No visible change yet — wiring is in place but no tabs consume it.)

- [ ] **Step 5: Commit**

```bash
git add src/app_state.rs
git commit -m "$(cat <<'EOF'
feat: start hook server and own AgentStatusStore in AppState
EOF
)"
```

---

### Task 6: Generate temp settings file per agent tab, substitute placeholder

**Files:**
- Modify: `src/agent_status/settings_file.rs`
- Modify: `src/agent_status/mod.rs`
- Modify: `tests/agent_status.rs`

- [ ] **Step 1: Write the failing tests for the temp-path / writer**

Append to `tests/agent_status.rs`:

```rust
use std::path::PathBuf;
use swrm::agent_status::{temp_settings_dir, write_settings_file};

#[test]
fn temp_settings_dir_is_pid_scoped() {
    let dir = temp_settings_dir();
    let pid = std::process::id();
    assert!(
        dir.file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.contains(&pid.to_string()))
            .unwrap_or(false),
        "expected temp dir to include pid {pid}, got {dir:?}",
    );
}

#[test]
fn write_settings_file_creates_parent_and_writes_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("tab-xyz.json");
    let json = r#"{"hooks":{}}"#;
    write_settings_file(&path, json).unwrap();
    let read_back = std::fs::read_to_string(&path).unwrap();
    assert_eq!(read_back, json);
}

#[test]
fn write_settings_file_overwrites_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("tab.json");
    write_settings_file(&path, "first").unwrap();
    write_settings_file(&path, "second").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test agent_status write_settings_file`
Expected: FAIL — symbols not found.

- [ ] **Step 3: Add the path / writer helpers**

Append to `src/agent_status/settings_file.rs`:

```rust
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// `${TMPDIR}/swrm-<pid>/` — the per-process directory we write tab settings
/// files into. PID-scoped so multiple swrm processes don't collide and so
/// the parent dir can be wiped on shutdown without affecting other tools.
/// Best-effort cleanup on app exit is not implemented; macOS wipes /tmp on
/// reboot which is sufficient for the MVP.
pub fn temp_settings_dir() -> PathBuf {
    std::env::temp_dir().join(format!("swrm-{}", std::process::id()))
}

/// Write `json` to `path`, creating the parent directory if necessary.
/// Overwrites any existing file. Returns the absolute path on success.
pub fn write_settings_file(path: &Path, json: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create settings parent {parent:?}"))?;
    }
    std::fs::write(path, json).with_context(|| format!("write settings file {path:?}"))?;
    Ok(())
}
```

- [ ] **Step 4: Re-export from mod.rs**

Edit `src/agent_status/mod.rs` — extend the `pub use settings_file::…` line:

```rust
pub use settings_file::{
    build_claude_settings_json, substitute_placeholder, temp_settings_dir, write_settings_file,
};
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test agent_status`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/agent_status/mod.rs src/agent_status/settings_file.rs tests/agent_status.rs
git commit -m "$(cat <<'EOF'
feat: temp settings file helpers under swrm-<pid> tmp dir
EOF
)"
```

---

### Task 7: Integrate per-tab settings file + register/unregister into `MainTabsPanel`

**Files:**
- Modify: `src/layout/main_tabs.rs`

- [ ] **Step 1: Identify the spawn site and add a tab_id allocator**

Read the existing file structure: `MainTabsPanel::spawn_tab` is where `Terminal::spawn_command` is called via `TerminalTab::new`. We need to (a) detect `$CLAUDE_SETTINGS` in the agent command, (b) generate a `tab_id`, (c) write the settings file, (d) substitute the placeholder before spawning, and (e) register the tab with `AgentStatusStore`. We also need `close_at` to call `unregister`.

Add a `tab_id: Option<String>` field on `TerminalTab` so `close_at` can look it up.

- [ ] **Step 2: Replace the `TerminalTab` struct + `TerminalTab::new` signature**

In `src/layout/main_tabs.rs`, change `pub struct TerminalTab` to include `tab_id`:

```rust
pub struct TerminalTab {
    pub label: String,
    pub terminal: Terminal,
    pub focus: FocusHandle,
    pub exited: bool,
    /// `Some` for agent tabs whose command had a `$CLAUDE_SETTINGS` placeholder;
    /// `None` for plain shells and agent tabs without the placeholder.
    pub tab_id: Option<String>,
}
```

Update the only construction call in `TerminalTab::new` to pass `tab_id: None`:

```rust
Ok(Self {
    label,
    terminal,
    focus,
    exited: false,
    tab_id: None,
})
```

Note: `TerminalTab::new` does NOT do placeholder substitution itself — `MainTabsPanel::spawn_tab` does the substitution and assigns `tab_id` after construction. This keeps `TerminalTab::new` free of `AgentStatusStore` knowledge.

- [ ] **Step 3: Add a private tab-id generator at the bottom of the file**

After the last `impl` block in `main_tabs.rs`, add:

```rust
fn next_tab_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    // Per-process counter is enough: tab_id is only used to dispatch hook
    // events back into the same swrm process, and the hook URLs include
    // the server port (also per-process).
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("tab-{n:08x}")
}
```

- [ ] **Step 4: Rewrite `spawn_tab` to do substitution and registration**

In `src/layout/main_tabs.rs`, replace the body of `MainTabsPanel::spawn_tab` with:

```rust
fn spawn_tab(&mut self, cwd: &Path, spec: TabSpec, base: &str, cx: &mut Context<Self>) {
    let entry = self.by_workspace.entry(cwd.to_path_buf()).or_default();
    let existing: Vec<&str> = entry
        .tabs
        .iter()
        .map(|t| t.read(cx).label.as_str())
        .collect();
    let label = swrm::tab_labels::unique_label(&existing, base);

    // Agent-status integration: if the agent command contains
    // `$CLAUDE_SETTINGS` (or the braced form), generate a per-tab
    // settings JSON, substitute the placeholder, and pre-register the
    // tab so the indicator can show before the first hook fires.
    let cwd_clone = cwd.to_path_buf();
    let (spec_for_spawn, tab_id_for_register) = match &spec {
        TabSpec::Agent(agent)
            if agent.command.contains("$CLAUDE_SETTINGS")
                || agent.command.contains("${CLAUDE_SETTINGS}") =>
        {
            let tab_id = next_tab_id();
            let origin = self.state.read(cx).agent_status_origin.clone();
            let json = swrm::agent_status::build_claude_settings_json(&origin, &tab_id);
            let path = swrm::agent_status::temp_settings_dir().join(format!("{tab_id}.json"));
            match swrm::agent_status::write_settings_file(&path, &json) {
                Ok(()) => {
                    let path_str = path.to_string_lossy().to_string();
                    let substituted = swrm::agent_status::substitute_placeholder(
                        &agent.command,
                        &path_str,
                    );
                    let mut substituted_agent = agent.clone();
                    substituted_agent.command = substituted;
                    (TabSpec::Agent(substituted_agent), Some(tab_id))
                }
                Err(err) => {
                    tracing::warn!(?err, "writing agent settings file failed; spawning without status tracking");
                    (spec.clone(), None)
                }
            }
        }
        _ => (spec.clone(), None),
    };

    let tab_id_for_struct = tab_id_for_register.clone();
    // Mirrors the existing `.unwrap()` here: tab-spawn failure is a
    // programming error (PTY config), not a runtime expectation.
    let tab = cx.new(|cx| {
        let mut t = TerminalTab::new(label, cwd_clone.clone(), &spec_for_spawn, cx).unwrap();
        t.tab_id = tab_id_for_struct;
        t
    });
    entry.tabs.push(tab);
    entry.active_index = entry.tabs.len() - 1;

    if let Some(tab_id) = tab_id_for_register {
        let store = self.state.read(cx).agent_status.clone();
        store.update(cx, |s, cx| s.register(tab_id, cwd_clone, cx));
    }
    cx.notify();
}
```

- [ ] **Step 5: Unregister in `close_at`**

Find `MainTabsPanel::close_at` and replace its body with:

```rust
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
    let removed = entry.tabs.remove(idx);
    if entry.active_index >= entry.tabs.len() && entry.active_index > 0 {
        entry.active_index = entry.tabs.len().saturating_sub(1);
    }
    if let Some(tab_id) = removed.read(cx).tab_id.clone() {
        let store = self.state.read(cx).agent_status.clone();
        store.update(cx, |s, cx| s.unregister(&tab_id, cx));
    }
    cx.notify();
}
```

- [ ] **Step 6: Build and smoke-run**

Run: `cargo build`
Expected: build succeeds.

Run `cargo run` and open a workspace. In Settings, add an agent named `claude` with command `echo "settings is $CLAUDE_SETTINGS" && sleep 30` (a stand-in that prints the substituted path so we can verify substitution without needing real Claude). Open a new tab via the `+` dropdown → click the agent.

Expected: the terminal shows `settings is /var/folders/.../swrm-<pid>/tab-00000000.json`, proving the placeholder was substituted before spawn.

Stop the app.

- [ ] **Step 7: Commit**

```bash
git add src/layout/main_tabs.rs
git commit -m "$(cat <<'EOF'
feat: substitute $CLAUDE_SETTINGS and register agent tabs with status store
EOF
)"
```

---

### Task 8: Left sidebar status indicator

**Files:**
- Modify: `src/layout/left_sidebar.rs`

- [ ] **Step 1: Subscribe to the store**

In `src/layout/left_sidebar.rs`, extend `LeftSidebarPanel` with a third subscription. Modify the struct:

```rust
pub struct LeftSidebarPanel {
    pub state: Entity<AppState>,
    list: Entity<ListState<WorkspacesDelegate>>,
    focus_handle: FocusHandle,
    _state_sub: Subscription,
    _store_sub: Subscription,
    _status_sub: Subscription,
}
```

And inside `LeftSidebarPanel::new`, after the existing `store_sub` line, add:

```rust
let status_store = state.read(cx).agent_status.clone();
let status_sub = cx.observe(&status_store, |this: &mut Self, _, cx| {
    this.list.update(cx, |list, cx| {
        cx.notify();
        list.delegate_mut().rebuild(cx);
    });
    cx.notify();
});
```

Then add `_status_sub: status_sub,` to the struct initializer.

- [ ] **Step 2: Render the indicator in `render_item`**

Inside `WorkspacesDelegate::render_item`, just before the `Some(ListItem::new(…)…)` block, compute the status:

```rust
let status = self
    .state
    .read(cx)
    .agent_status
    .read(cx)
    .status_for_workspace(&ws.path);
```

Then wrap the label `div` with an h-flex that prefixes a colored dot when status is `Some`. Replace the `.child(div().id(("workspace-content", row_id)).w_full().child(ws.label.clone()).context_menu(...))` block with:

```rust
.child(
    h_flex()
        .id(("workspace-content", row_id))
        .w_full()
        .items_center()
        .gap_2()
        .when_some(status, |this, status| {
            this.child(status_dot(status))
        })
        .child(div().flex_1().child(ws.label.clone()))
        .context_menu(move |menu, _window, _cx| {
            let state = menu_state.clone();
            let path = menu_path.clone();
            let label = menu_label.clone();
            let linked = is_linked_worktree;
            menu.item(PopupMenuItem::new(label).on_click(move |_ev, window, cx| {
                close_workspace(&path, linked, &state, window, cx);
            }))
        }),
)
```

Note: `when_some` is from `gpui::prelude::FluentBuilder`. Add `use gpui::prelude::FluentBuilder;` to the imports at the top of the file if it's not already there.

- [ ] **Step 3: Add the `status_dot` helper at the bottom of the file**

After the last `fn` in `left_sidebar.rs`, add:

```rust
fn status_dot(status: swrm::agent_status::AgentStatus) -> impl IntoElement {
    use swrm::agent_status::AgentStatus;
    let color: u32 = match status {
        // Amber (needs attention), green (just finished), blue (active),
        // gray (alive but quiet). Picked to be readable on the dark theme;
        // the theme doesn't expose semantically named colors we can map to,
        // so values are inline.
        AgentStatus::Notify => 0xFFB020,
        AgentStatus::Done => 0x46A758,
        AgentStatus::Working => 0x3E63DD,
        AgentStatus::Idle => 0x7B7B7B,
    };
    div()
        .w(gpui::px(8.))
        .h(gpui::px(8.))
        .rounded_full()
        .bg(gpui::rgb(color))
        .flex_none()
}
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: build succeeds. Fix any missing imports surfaced by the compiler.

- [ ] **Step 5: Manual verification — fake hook delivery**

Run the app: `cargo run`. Open a workspace. From the swrm app open a regular shell tab and POST a fake event to the hook server. First find the port — the app does NOT print it by default, so add `tracing::info!(port, "agent-status server listening");` next to the `format!("http://127.0.0.1:{port}")` line in `app_state.rs` (temporarily — remove before committing this task).

Run `cargo run` again, look at stderr for the port, then in another terminal:

```sh
PORT=<port from log>
# Pretend tab-00000000 is the currently-open agent tab. Open an agent tab in
# the swrm UI first using the $CLAUDE_SETTINGS-bearing agent from Task 7's
# smoke test, then read its tab_id from the file name under /tmp/swrm-<pid>/.
TAB_ID=tab-00000000
curl -X POST "http://127.0.0.1:$PORT/event/$TAB_ID/notify"
```

Expected: the workspace row in the left sidebar gets an amber dot. Run the same with `working`, `done`, `idle` and confirm the dot color changes accordingly. Close the tab (`cmd-w`) and confirm the dot disappears.

Remove the temporary `tracing::info!` log line before committing.

- [ ] **Step 6: Commit**

```bash
git add src/layout/left_sidebar.rs
git commit -m "$(cat <<'EOF'
feat: show colored agent-status dot beside each workspace in the sidebar
EOF
)"
```

---

### Task 9: Settings view hint for the placeholder

**Files:**
- Modify: `src/layout/settings_view.rs`

- [ ] **Step 1: Add a hint label under the Command input**

In `src/layout/settings_view.rs`, find the block that renders the Command row inside `SettingItem::render`:

```rust
.child(
    h_flex()
        .gap_2()
        .child(Label::new("Command"))
        .child(Input::new(&command_input)),
)
```

Replace it with:

```rust
.child(
    v_flex()
        .gap_1()
        .child(
            h_flex()
                .gap_2()
                .child(Label::new("Command"))
                .child(Input::new(&command_input)),
        )
        .child(
            div()
                .pl(gpui::px(64.))
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Use $CLAUDE_SETTINGS to enable status tracking"),
        ),
)
```

Note: the existing closure parameter for `SettingItem::render` is `move |_opts, _window, _cx|`. Rename `_cx` to `cx` so the theme lookup compiles. `gpui_component::ActiveTheme` is already pulled in via the existing imports (it's the trait that supplies the `cx.theme()` method); if not, add `use gpui_component::ActiveTheme;` to the imports at the top.

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: build succeeds.

- [ ] **Step 3: Manual verification**

Run `cargo run`, open Settings (`cmd-,`), click "+ Add agent". Confirm the hint text appears under the Command input.

- [ ] **Step 4: Commit**

```bash
git add src/layout/settings_view.rs
git commit -m "$(cat <<'EOF'
feat: hint that $CLAUDE_SETTINGS enables status tracking
EOF
)"
```

---

### Task 10: Final integration check + format pass

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: all tests PASS, including the new `agent_status` suite and the existing `workspace_persistence`, `git_status`, `vt_smoke`, etc.

- [ ] **Step 2: Format the tree**

Run: `cargo fmt`
Expected: no errors; any newly-introduced files get formatted. If `cargo fmt` produces a diff, commit it.

- [ ] **Step 3: End-to-end check against real Claude Code (manual)**

Install Claude Code if not already present. Configure a swrm agent named `claude` with command `claude --settings $CLAUDE_SETTINGS`. Open a workspace, open a new claude tab.

Expected sequence:
1. Sidebar dot appears gray (Idle) within ~1 second (Claude's `SessionStart` fires).
2. Type a prompt → dot turns blue (Working) while Claude is thinking.
3. Trigger a permission prompt (e.g. ask Claude to run `rm`) → dot turns amber (Notify).
4. Resolve the prompt → dot returns to blue (Working) then green (Done) when the turn completes.
5. Close the tab → dot disappears.

If any transition is missing, inspect `tracing::debug!` output for ignored statuses and check the generated `/tmp/swrm-<pid>/<tab_id>.json` file matches the hook list in `build_claude_settings_json`.

- [ ] **Step 4: Commit any formatting changes**

```bash
git status
# If cargo fmt produced changes:
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt after agent-status integration
EOF
)"
```

---

## Notes & non-goals (for the executor)

- **No `Unknown` status variant.** Hook events with an unrecognized wire string are dropped at the channel-pump boundary (Task 5). The current set is sufficient.
- **No temp-dir cleanup on exit.** macOS clears `/tmp` on reboot; per-tab files are tiny. Adding a `Drop` impl is fine if a later need arises but not required.
- **No tab-level UI indicator** (yet). MVP scope is sidebar-only. The `tab_id` on `TerminalTab` is in place if we add one later.
- **No multi-agent placeholder support** (yet). Only `$CLAUDE_SETTINGS` / `${CLAUDE_SETTINGS}`. Other agents (pi, opencode) would each get their own placeholder + file generator in `settings_file.rs`, mirroring agent-status's `build_extension` pattern.
- **`curl` is a runtime requirement.** macOS ships it; if a future Linux build hits a `curl`-less environment, the hooks no-op silently (Claude doesn't care about hook exit codes), and the indicator just won't update — degraded but non-fatal.
