# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What `swrm` is

A native Rust desktop app for managing multiple Git worktrees side-by-side, inspired by Superset. The window has a left sidebar (workspace switcher, grouped by project), a right sidebar (`git status` + unified diff for the active workspace), and a central tabbed area that hosts a real terminal per workspace. Workspaces are git worktrees; each workspace keeps its own set of terminal tabs.

`docs/superpowers/plans/2026-05-17-swrm-mvp.md` is the implementation plan the app was built from. It describes intent task-by-task; the code is the source of truth where they diverge.

## Common commands

| Task | Command |
| --- | --- |
| Build | `cargo build` |
| Run the app | `cargo run` |
| Run all tests | `cargo test` |
| Run a single test file | `cargo test --test <name>` (e.g. `cargo test --test vt_smoke`) |
| Run a single test function | `cargo test <fn_name>` |
| Format | `cargo fmt` (the whole tree is kept rustfmt-clean) |

## Architecture in one breath

Single-process gpui app. `main.rs` wires the runtime; `app::Root` is the only top-level view (wrapped inside `gpui_component::Root`, which gpui-component widgets look up for tooltips/notifications and **for shaping text** — without that wrapper, text doesn't render). `Root` owns `AppState` (active workspace + sidebar visibility + the `WorkspaceStore` entity) and a `Layout` struct exposing the `DockArea` and the three panel entities.

The three panels are gpui-component `Panel`s living in a `DockArea`:

- `layout::left_sidebar::LeftSidebarPanel` — uses `gpui_component::list::{List, ListState, ListDelegate}` with a `WorkspacesDelegate` that groups `Workspace`s by `project_dir` (parent of `gix::Repository::common_dir()`) into sections.
- `layout::right_sidebar::RightSidebarPanel` — subscribes to `AppEvent::ActiveWorkspaceChanged`, calls `git::collect_status` on the background executor, renders a click-to-diff list and a refresh button.
- `layout::main_tabs::MainTabsPanel` — owns `HashMap<PathBuf, WorkspaceTabs>` keyed by workspace path, so each workspace keeps its own terminal tab set (`ensure_tab_for_active` on workspace change spawns a tab only if that workspace has none). The visible tab strip is `gpui_component::tab::TabBar`; the inner `TerminalTab` view spawns an async task that consumes `alacritty_terminal::event::Event`s from the PTY and notifies gpui to re-render on terminal activity.

`workspace::Workspace` (in `src/workspace/mod.rs`) is `{ label, path, branch, project: Option<PathBuf> }`. The `project` field is `None` for entries persisted before it was introduced — always go through `Workspace::project_dir()` which falls back to `path`.

`terminal::Terminal` wraps a `Backend` that owns `alacritty_terminal::term::Term<SwrmListener>` and an `EventLoop` powering `alacritty_terminal::tty::new`. Events (cursor moves, mode changes, bell, exit) flow through `SwrmListener` as an unbounded mpsc channel. Snapshots are fetched via `Terminal::snapshot()`, which locks the term and iterates cells, then rasters them as one `div` per row in JetBrains Mono in `terminal::render::render_snapshot`.

`AppState`, `WorkspaceStore`, panels, and `TerminalTab` all use the standard gpui pattern: `cx.subscribe(&entity, |this, _, event, cx| …)` for typed events, `cx.observe(&entity, |this, _, cx| cx.notify())` for general "something changed" reactivity. Always store the returned `Subscription` on the view struct or it cancels.

Persistence: `WorkspaceStore` auto-saves the workspace list to `~/Library/Application Support/swrm/workspaces.json` (or `dirs::config_dir()` equivalent) on `add`/`remove`. Nothing else persists — open tabs, sidebar visibility, etc. are in-memory.

## Working on the UI

**Prefer existing `gpui-component` widgets over hand-rolling `div`-based equivalents.** When you reach for a tabs strip, a list, a button, a popover, a tooltip — first check `~/.cargo/git/checkouts/gpui-component-<hash>/<rev>/crates/ui/src/` and `crates/story/src/stories/` for a worked example. Recent rewrites swapped a custom div-tab strip for `TabBar` and a custom div-list for `List` precisely because the design pacing matters and the components match the rest of the UI.

A few non-obvious facts about the gpui/gpui-component pinned revisions in this repo:

- `gpui_platform` **must** include the `font-kit` feature (see `Cargo.toml`). Without it the macOS text system silently fails to load fonts and the whole UI renders without text.
- `Theme.font_family` defaults to `.SystemUIFont`, an Apple alias `font_kit` can't resolve. We bundle `IBMPlexSans-Regular.ttf` + `IBMPlexSans-SemiBold.ttf` + `JetBrainsMono-Regular.ttf` under `assets/fonts/` and register them via `cx.text_system().add_fonts(...)`, then override `Theme.font_family` / `mono_font_family` in `main.rs`. Bundle-by-include-bytes; don't rely on system font resolution.
- gpui-component widgets look up `window.root::<gpui_component::Root>()` for text shaping, dialogs, etc. The window's first view **must** be `gpui_component::Root::new(view, window, cx)` — see `main.rs`. Skipping that wrapper makes widget text invisible.
- gpui-component's `Panel` trait uses `panel_name() -> &'static str` (not `panel_id`), and has `Focusable` + `EventEmitter<PanelEvent>` as supertraits. Each panel needs its own `FocusHandle` field.
- The single-title path in `gpui_component::dock::tab_panel::render_title_bar` hard-codes `h(px(30.))` with no bottom border. Custom-height titles or a border-bottom under the panel header would require patching gpui-component; the public `Panel` trait doesn't expose either knob.
- gpui-component always renders the `…` toolbar button on every panel; you can disable its zoom menu via `fn zoomable(&self, _cx) -> None`, but you can't hide the button itself without patching gpui-component.
- The OS title bar is configured `appears_transparent: true` via `TitleBar::title_bar_options()` so the macOS traffic lights overlay the first row of content. There is **no** dedicated app title bar widget — Zed-style, the first row of panel content provides the visible chrome.

## Tests

Three integration test files under `tests/`:

- `workspace_persistence.rs` — JSON round-trip of `Workspace`. **If you add a field to `Workspace`, update the fixture in this test.**
- `git_status.rs` — boots a real temp-dir git repo via `std::process::Command` and asserts our `gix::status` mapping handles Modified/Added/Untracked.
- `vt_smoke.rs` — feeds bytes into `VtWrapper` and asserts the snapshot grid + cursor.

There are no unit tests inside `src/`; lean on the integration tests when refactoring the modules they cover.

## Conventions to keep

- Run `cargo fmt` before committing. The tree is kept rustfmt-clean and CI / future reviewers will notice churn.
- Public API at the top of each file, private items at the bottom.
- `gix` is the default for read-only git operations (`open`, `head_name`, `worktrees`, `status`); shell out to `git` only for operations gix doesn't cleanly support — currently `git worktree add` (in `workspace::worktree::create_worktree`) and `git diff` (in `git::diff::diff_file`).
- Commits use Conventional-Commit-ish prefixes (`feat:`, `fix:`, `chore:`, `build:`, `refactor:`, `docs:`); see `git log` for tone.
