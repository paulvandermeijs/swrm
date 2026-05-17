# Alacritty Terminal Backend Migration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `libghostty-vt` + `portable-pty` with `alacritty_terminal` (crates.io 0.26) as a single cutover, gaining per-cell ANSI colors, scrollback, a full keymap, and event-driven re-render — and dropping the Zig 0.15.2 build requirement.

**Architecture:** `alacritty_terminal::tty::new` opens the PTY and forks the shell. `EventLoop::spawn()` owns the reader thread, parses bytes through `vte` into an `Arc<FairMutex<Term<SwrmListener>>>`, and emits `Event`s through our `SwrmListener` onto a `futures::channel::mpsc::unbounded` sender. The receiving side runs as a gpui async task on `TerminalTab` that drains events and calls `cx.notify()` on `Wakeup` / `Bell` / `Title` / `ChildExit`. Rendering walks `term.renderable_content()`, batches cells into per-style runs, and emits one `div` per run with `bg`/`text_color` set from a 256-color ANSI palette. The 16 ms polling tick goes away.

**Tech Stack:** Rust 2024, gpui (Zed fork), gpui-component, `alacritty_terminal = "0.26"`, `futures = "0.3"`, JetBrains Mono font.

**Reference doc:** `docs/superpowers/research/2026-05-17-terminal-backends.md`. Zed's reference shapes live in `zed-industries/zed` at `crates/terminal/src/terminal.rs`, `crates/terminal/src/mappings/keys.rs`, `crates/terminal_view/src/terminal_element.rs`.

**Scope (in this plan):** dependency swap, PTY + event-loop wiring, headless smoke test, gpui event bridge, per-cell colors with bold/italic/underline, batched-run renderer, full keymap port (incl. app-cursor / app-keypad / F-keys / alt-meta / bracketed paste), mouse-wheel scrollback.

**Out of scope (deliberately deferred):** selection + clipboard copy, mouse reporting to the app, OSC 8 hyperlink rendering, IME composition, resize-to-element-bounds (the app is already hardcoded to 80×24; that orthogonal limitation stays).

---

## File Structure

**Create:**
- `src/terminal/snapshot.rs` — new `Snapshot`, `Cell { ch, fg: u32, bg: u32, flags: CellFlags }`, `CellFlags` (bold/italic/underline/strikethrough/inverse/dim).
- `src/terminal/listener.rs` — `SwrmListener` (impl of `alacritty_terminal::event::EventListener`) and the receiver type alias.
- `src/terminal/backend.rs` — owns the `Arc<FairMutex<Term<SwrmListener>>>`, the PTY, the `EventLoop` join handle, and the `EventLoopSender` (input/resize/shutdown channel). Construction of `tty::Options` lives here.
- `src/terminal/colors.rs` — `const ANSI_256: [Rgb; 256]` palette + `resolve_color(Color, is_fg) -> Rgb` helper.
- `tests/input_keymap.rs` — pure-function tests for `input::encode`.

**Modify:**
- `Cargo.toml` — drop `libghostty-vt`, `portable-pty`; add `alacritty_terminal = "0.26"`, `futures = "0.3"`.
- `src/terminal/mod.rs` — re-shape `Terminal` to own a `Backend` plus an events `UnboundedReceiver`; expose `spawn`, `write_input`, `resize`, `scroll`, `snapshot`, `subscribe_events`, `mode`.
- `src/terminal/input.rs` — rewrite to port Zed's `to_esc_str` shape; takes `TermMode`, handles F1-F20, alt-meta, app-cursor/app-keypad, bracketed paste.
- `src/terminal/render.rs` — rewrite to walk row by row, batch contiguous cells of identical `(fg, bg, flags)`, emit one styled `div` per run.
- `src/layout/main_tabs.rs:25-48` — delete the 16 ms tick; replace with `cx.spawn` that awaits the events stream and `cx.notify()`s.
- `tests/vt_smoke.rs` — rewrite against the new headless `Terminal::spawn_headless` (or `Backend::headless`) that takes no PTY.
- `CLAUDE.md` — remove "First-build setup (macOS)" section.

**Delete:**
- `src/terminal/vt.rs`
- `src/terminal/pty.rs`

**Responsibility split:**
- `mod.rs` is the public surface (`Terminal`). Nothing in `src/` outside `terminal/` should know about alacritty types.
- `backend.rs` is alacritty-coupled but PTY-and-event-loop only — no rendering, no input encoding.
- `snapshot.rs` is pure data — no gpui, no alacritty (built from alacritty in `backend.rs`).
- `colors.rs` is pure data.
- `input.rs` reads `TermMode` (an alacritty type) and emits bytes — minimal coupling.
- `render.rs` is gpui-coupled but reads only `Snapshot`.

---

## Cross-cutting notes for the implementer

- **The exact `alacritty_terminal` 0.26 signatures must be verified against https://docs.rs/alacritty_terminal/0.26.0/** as you go. The code blocks in this plan reflect the shape reported by research and Zed's current code, but field names on `tty::Options` and method names on `Term`/`Notifier`/`EventLoopSender` may shift by one or two letters; cross-check before each implementation step. If a signature drifts from what's shown, prefer docs.rs over this plan.
- **Compile after every task** with `cargo build`. If a task is going to break the build, that's called out in its description. The default is "every commit compiles."
- **The first cell-color cell you see in a real shell should be the prompt's color** — that's your end-to-end signal that fg + bg + the 256-palette + run batching all work.
- **TDD discipline:** unit tests live in `tests/`. The headless smoke test (`tests/vt_smoke.rs`) is your TDD target — it exercises `Terminal` + `Backend` + snapshot without spawning a child process. Renderer + gpui-event-bridge are not unit-testable; they get manual smoke at the end.

---

## Task 1: Swap dependencies in Cargo.toml

**Files:**
- Modify: `Cargo.toml`

This task changes only dependencies. The crate will not build after this step — that's fine; subsequent tasks fix it. The reason for doing the dep swap up-front: every later task imports `alacritty_terminal::*`.

- [ ] **Step 1: Edit Cargo.toml — remove old, add new**

Open `Cargo.toml`. Remove these two lines from `[dependencies]`:

```toml
libghostty-vt = { git = "https://github.com/Uzaaft/libghostty-rs" }
portable-pty = "0.9"
```

Add these two:

```toml
alacritty_terminal = "0.26"
futures = "0.3"
```

Leave everything else untouched. The resulting `[dependencies]` block:

```toml
[dependencies]
anyhow = "1"
dirs = "6"
gix = { version = "0.83", default-features = false, features = ["sha1", "status", "worktree-mutation", "blocking-network-client"] }
gpui = { git = "https://github.com/zed-industries/zed" }
gpui_platform = { git = "https://github.com/zed-industries/zed", features = ["font-kit", "runtime_shaders"] }
gpui-component = { git = "https://github.com/longbridge/gpui-component", branch = "main" }
gpui-component-assets = { git = "https://github.com/longbridge/gpui-component", branch = "main" }
alacritty_terminal = "0.26"
futures = "0.3"
petname = { version = "3", default-features = false, features = ["default-rng", "default-words"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 2: Verify the dependency tree resolves**

Run: `cargo update -p alacritty_terminal`
Expected: no errors. `alacritty_terminal v0.26.x` resolved. If a newer 0.26.x patch is published, take it.

`cargo build` will fail at this point — expected (referenced modules don't exist yet). Do **not** commit yet; this task ends with the next step.

- [ ] **Step 3: Stage but do not commit**

Run: `git add Cargo.toml Cargo.lock`
Do not run `git commit` — Task 2 finishes the compile and commits together. Leaving a non-building commit on master is forbidden.

---

## Task 2: Introduce `snapshot.rs` with the new shape

**Files:**
- Create: `src/terminal/snapshot.rs`
- Modify: `src/terminal/mod.rs` (add `pub mod snapshot;` and re-exports)

This is pure data — no alacritty imports, no gpui imports. Tests can construct it manually.

- [ ] **Step 1: Write `src/terminal/snapshot.rs`**

```rust
use bitflags::bitflags;

bitflags! {
    /// Per-cell display attributes. Subset of alacritty's `term::cell::Flags`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CellFlags: u8 {
        const BOLD          = 0b0000_0001;
        const ITALIC        = 0b0000_0010;
        const UNDERLINE     = 0b0000_0100;
        const STRIKEOUT     = 0b0000_1000;
        const INVERSE       = 0b0001_0000;
        const DIM           = 0b0010_0000;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    /// 0x00RRGGBB
    pub fg: u32,
    /// 0x00RRGGBB
    pub bg: u32,
    pub flags: CellFlags,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cols: u16,
    pub rows: u16,
    /// Row-major; `len == cols * rows`.
    pub cells: Vec<Cell>,
    pub cursor_row: u16,
    pub cursor_col: u16,
    /// True if the cursor should be drawn (i.e. terminal is not in DECTCEM-hidden mode).
    pub cursor_visible: bool,
}

impl Snapshot {
    pub fn cell_at(&self, row: u16, col: u16) -> Option<&Cell> {
        let idx = row as usize * self.cols as usize + col as usize;
        self.cells.get(idx)
    }
}
```

`bitflags` is already in the transitive tree (used by gpui and alacritty); it doesn't need an explicit Cargo.toml entry — re-export comes through `alacritty_terminal`. If `cargo build` complains, add `bitflags = "2"` to `[dependencies]`.

- [ ] **Step 2: Wire it into `src/terminal/mod.rs`**

Open `src/terminal/mod.rs`. Replace the whole file with the temporary shell below — it will not compile end-to-end until Task 4, but each task in between adds one piece. **This is the only file allowed to be in a broken state mid-migration**; everything else is added or modified atomically.

```rust
pub mod input;
pub mod render;
pub mod snapshot;
// Added in later tasks:
//   pub mod backend;
//   pub mod colors;
//   pub mod listener;

pub use snapshot::{Cell, CellFlags, Snapshot};
```

`vt.rs` and `pty.rs` are still on disk — leave them; we delete them in Task 4 once the new code compiles.

- [ ] **Step 3: Confirm the snapshot module compiles in isolation**

Run: `cargo build --lib 2>&1 | head -40`
Expected: errors about `vt::*` / `pty::*` references elsewhere — but **no** errors emanating from `snapshot.rs` itself. Confirm `snapshot.rs` is clean before moving on.

- [ ] **Step 4: Do not commit yet**

The first commit lands at the end of Task 5 once the backend wiring, tests, and `TerminalTab` rewrite are all in. Continue to Task 3.

---

## Task 3: Build the alacritty backend (`backend.rs` + `listener.rs` + `colors.rs`)

**Files:**
- Create: `src/terminal/listener.rs`
- Create: `src/terminal/colors.rs`
- Create: `src/terminal/backend.rs`
- Modify: `src/terminal/mod.rs` (wire the new modules, replace `Terminal` impl)

This is the heavy task. It introduces the alacritty wiring (PTY + Term + EventLoop) and the headless variant used by tests. After this task, the crate compiles again and `cargo test --test vt_smoke` (rewritten in Task 4) is green.

### Step-by-step

- [ ] **Step 1: Write `src/terminal/listener.rs`**

```rust
use alacritty_terminal::event::{Event as AlacEvent, EventListener};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// Forwards alacritty events from the event-loop thread onto a futures channel
/// the gpui task awaits. `send_event` is called on the alacritty thread.
#[derive(Clone)]
pub struct SwrmListener {
    tx: UnboundedSender<AlacEvent>,
}

impl SwrmListener {
    pub fn pair() -> (Self, UnboundedReceiver<AlacEvent>) {
        let (tx, rx) = unbounded();
        (Self { tx }, rx)
    }
}

impl EventListener for SwrmListener {
    fn send_event(&self, event: AlacEvent) {
        // Receiver dropped means the Terminal was destroyed; ignore.
        let _ = self.tx.unbounded_send(event);
    }
}
```

- [ ] **Step 2: Write `src/terminal/colors.rs`**

```rust
use alacritty_terminal::vte::ansi::{NamedColor, Rgb};

/// Standard 16 ANSI colors + 240 xterm 256-color palette entries.
/// Index 0-15: ANSI 16; 16-231: 6×6×6 cube; 232-255: grayscale.
pub const ANSI_256: [Rgb; 256] = build_palette();

const fn build_palette() -> [Rgb; 256] {
    let mut p = [Rgb { r: 0, g: 0, b: 0 }; 256];

    // ANSI 16 (xterm defaults).
    let base = [
        (0x00, 0x00, 0x00), (0xcd, 0x00, 0x00), (0x00, 0xcd, 0x00), (0xcd, 0xcd, 0x00),
        (0x00, 0x00, 0xee), (0xcd, 0x00, 0xcd), (0x00, 0xcd, 0xcd), (0xe5, 0xe5, 0xe5),
        (0x7f, 0x7f, 0x7f), (0xff, 0x00, 0x00), (0x00, 0xff, 0x00), (0xff, 0xff, 0x00),
        (0x5c, 0x5c, 0xff), (0xff, 0x00, 0xff), (0x00, 0xff, 0xff), (0xff, 0xff, 0xff),
    ];
    let mut i = 0;
    while i < 16 {
        p[i] = Rgb { r: base[i].0, g: base[i].1, b: base[i].2 };
        i += 1;
    }

    // 6×6×6 RGB cube (indices 16-231).
    let levels = [0u8, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
    let mut r = 0;
    while r < 6 {
        let mut g = 0;
        while g < 6 {
            let mut b = 0;
            while b < 6 {
                let idx = 16 + 36 * r + 6 * g + b;
                p[idx] = Rgb { r: levels[r], g: levels[g], b: levels[b] };
                b += 1;
            }
            g += 1;
        }
        r += 1;
    }

    // Grayscale (232-255).
    let mut k = 0;
    while k < 24 {
        let v = 8 + 10 * k as u8;
        p[232 + k] = Rgb { r: v, g: v, b: v };
        k += 1;
    }
    p
}

pub const DEFAULT_FG: Rgb = Rgb { r: 0xee, g: 0xee, b: 0xee };
pub const DEFAULT_BG: Rgb = Rgb { r: 0x11, g: 0x11, b: 0x11 };

pub fn resolve_named(name: NamedColor) -> Rgb {
    // The named-color enum has variants that map to indices 0-15 (plus a few
    // semantic specials like Foreground/Background). Treat any non-indexed
    // semantic name as a sensible default — refine if a real shell looks wrong.
    use NamedColor::*;
    match name {
        Foreground => DEFAULT_FG,
        Background => DEFAULT_BG,
        Cursor => DEFAULT_FG,
        Black => ANSI_256[0], Red => ANSI_256[1], Green => ANSI_256[2], Yellow => ANSI_256[3],
        Blue => ANSI_256[4], Magenta => ANSI_256[5], Cyan => ANSI_256[6], White => ANSI_256[7],
        BrightBlack => ANSI_256[8], BrightRed => ANSI_256[9], BrightGreen => ANSI_256[10],
        BrightYellow => ANSI_256[11], BrightBlue => ANSI_256[12], BrightMagenta => ANSI_256[13],
        BrightCyan => ANSI_256[14], BrightWhite => ANSI_256[15],
        // Cross-check the full NamedColor enum on docs.rs; fall through to fg/bg.
        BrightForeground => DEFAULT_FG,
        DimBlack | DimRed | DimGreen | DimYellow | DimBlue | DimMagenta | DimCyan | DimWhite => {
            DEFAULT_FG
        }
    }
}

#[inline]
pub fn rgb_to_u32(c: Rgb) -> u32 {
    ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}
```

If `NamedColor` has variants this match doesn't list (it almost certainly does — verify against docs.rs for 0.26), add them with `_ => DEFAULT_FG`. Don't leave the match non-exhaustive.

- [ ] **Step 3: Write `src/terminal/backend.rs` — type definition + headless constructor**

```rust
use crate::terminal::colors::{ANSI_256, DEFAULT_BG, DEFAULT_FG, resolve_named, rgb_to_u32};
use crate::terminal::listener::SwrmListener;
use crate::terminal::snapshot::{Cell, CellFlags, Snapshot};
use alacritty_terminal::event::{Event as AlacEvent, EventLoopSender, Notify, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags as AlacFlags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb};
use anyhow::{Context, Result};
use futures::channel::mpsc::UnboundedReceiver;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;

/// Dimensions impl alacritty wants. cols/lines in cells, total_lines includes scrollback.
#[derive(Clone, Copy)]
pub struct TermSize {
    pub columns: usize,
    pub screen_lines: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.screen_lines + 10_000
    }
    fn screen_lines(&self) -> usize {
        self.screen_lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

impl From<TermSize> for WindowSize {
    fn from(s: TermSize) -> Self {
        WindowSize {
            num_lines: s.screen_lines as u16,
            num_cols: s.columns as u16,
            cell_width: 0,
            cell_height: 0,
        }
    }
}

pub struct Backend {
    pub term: Arc<FairMutex<Term<SwrmListener>>>,
    pub tx: EventLoopSender,
    /// Held to keep the event-loop thread alive.
    _join: Option<JoinHandle<(EventLoop<tty::Pty, SwrmListener>, alacritty_terminal::event_loop::State)>>,
    size: TermSize,
}

impl Backend {
    /// Spawn the shell in `cwd` with `cols` × `rows`. Returns the backend and
    /// the event receiver to bridge into gpui (Wakeup, Bell, Title, ChildExit…).
    pub fn spawn(
        cwd: &Path,
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
        tty_opts.shell = Some(tty::Shell::new(shell, vec![]));
        tty_opts.working_directory = Some(cwd.to_path_buf());
        tty_opts.drain_on_exit = true;

        let pty = tty::new(&tty_opts, size.into(), /* window_id */ 0)
            .context("open pty + fork shell")?;

        let event_loop = EventLoop::new(
            term.clone(),
            listener,
            pty,
            /* drain_on_exit */ true,
            /* ref_test */ false,
        )
        .context("build alacritty event loop")?;

        let tx = event_loop.channel();
        let _join = Some(event_loop.spawn());

        Ok((
            Self {
                term,
                tx,
                _join,
                size,
            },
            events_rx,
        ))
    }

    /// Headless variant — no PTY, no event loop. Caller drives the Term
    /// directly through `with_term_mut` and feeds via `processor` (alacritty
    /// exposes `Term::input` etc. for unit tests). Used by `tests/vt_smoke.rs`.
    pub fn headless(cols: u16, rows: u16) -> (Self, UnboundedReceiver<AlacEvent>) {
        let size = TermSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let (listener, events_rx) = SwrmListener::pair();
        let term = Term::new(Config::default(), &size, listener);
        let term = Arc::new(FairMutex::new(term));
        // No tx — sending input on a headless backend panics. We won't use it.
        // EventLoopSender has no public no-op constructor; for tests we keep
        // `tx` as `None` via an enum, OR we expose a separate `HeadlessBackend`.
        // Simplest: make `tx` an `Option<EventLoopSender>` on `Backend`.
        unimplemented!("see Step 3a below — make `tx` Option<EventLoopSender>")
    }

    pub fn size(&self) -> TermSize {
        self.size
    }

    pub fn mode(&self) -> TermMode {
        self.term.lock().mode()
    }
}
```

- [ ] **Step 3a: Adjust `Backend` so headless is sound**

Replace `pub tx: EventLoopSender` with `pub tx: Option<EventLoopSender>`. Methods that send (Task 4) check it. Update both constructors. Final shape:

```rust
pub struct Backend {
    pub term: Arc<FairMutex<Term<SwrmListener>>>,
    pub tx: Option<EventLoopSender>,
    _join: Option<JoinHandle<(EventLoop<tty::Pty, SwrmListener>, alacritty_terminal::event_loop::State)>>,
    size: TermSize,
}

impl Backend {
    pub fn headless(cols: u16, rows: u16) -> (Self, UnboundedReceiver<AlacEvent>) {
        let size = TermSize { columns: cols as usize, screen_lines: rows as usize };
        let (listener, events_rx) = SwrmListener::pair();
        let term = Term::new(Config::default(), &size, listener);
        let term = Arc::new(FairMutex::new(term));
        (Self { term, tx: None, _join: None, size }, events_rx)
    }
}
```

- [ ] **Step 3b: Add `Backend::feed_headless` for tests**

In `backend.rs`, add a test-only-shaped method that drives `vte` directly against the Term without a PTY. This is the same call path the EventLoop uses internally.

```rust
impl Backend {
    /// Feed raw bytes through the vte parser into the Term. Synchronous;
    /// only valid for headless backends.
    pub fn feed_bytes(&self, bytes: &[u8]) {
        use alacritty_terminal::vte::Parser;
        use alacritty_terminal::Processor;
        // Cross-check exact processor type on docs.rs for 0.26 — alacritty
        // exposes `term::Processor` (or similar) that wraps a vte::Parser
        // and calls term.handle_action / term.input. Zed wraps this in
        // `crates/terminal/src/terminal.rs`.
        let mut parser: Parser = Parser::new();
        let mut term = self.term.lock();
        let mut processor = Processor::new();
        for &b in bytes {
            processor.advance(&mut *term, b);
        }
        let _ = parser; // suppress unused if Processor handles parsing itself.
    }
}
```

Note: alacritty 0.26 exposes the byte→Term plumbing through `alacritty_terminal::vte::Parser` driven by a `Processor` (or `Performer`) struct. The implementer should verify on docs.rs which combinator alacritty exports for embedders that want to drive bytes synchronously. If neither is public, fall back to importing `vte` directly and implementing `vte::Perform` on the Term wrapper — but Zed's tests in `crates/terminal/src/terminal_tests.rs` do this through a public alacritty API, so look there for the canonical shape.

- [ ] **Step 3c: Add the snapshot extractor on `Backend`**

```rust
impl Backend {
    pub fn snapshot(&self) -> Snapshot {
        let term = self.term.lock();
        let content = term.renderable_content();
        let cols = self.size.columns as u16;
        let rows = self.size.screen_lines as u16;

        let capacity = cols as usize * rows as usize;
        let mut cells: Vec<Cell> = vec![
            Cell {
                ch: ' ',
                fg: rgb_to_u32(DEFAULT_FG),
                bg: rgb_to_u32(DEFAULT_BG),
                flags: CellFlags::empty(),
            };
            capacity
        ];

        for indexed in content.display_iter {
            let row = (indexed.point.line.0 + content.display_offset as i32) as i32;
            let row_visible = row.max(0) as usize;
            if row_visible >= rows as usize {
                continue;
            }
            let col = indexed.point.column.0;
            let idx = row_visible * cols as usize + col;

            let alac_flags = indexed.cell.flags;
            let mut flags = CellFlags::empty();
            if alac_flags.contains(AlacFlags::BOLD) { flags |= CellFlags::BOLD; }
            if alac_flags.contains(AlacFlags::ITALIC) { flags |= CellFlags::ITALIC; }
            if alac_flags.intersects(AlacFlags::ALL_UNDERLINES) { flags |= CellFlags::UNDERLINE; }
            if alac_flags.contains(AlacFlags::STRIKEOUT) { flags |= CellFlags::STRIKEOUT; }
            if alac_flags.contains(AlacFlags::INVERSE) { flags |= CellFlags::INVERSE; }
            if alac_flags.contains(AlacFlags::DIM) { flags |= CellFlags::DIM; }

            let mut fg = resolve_color(indexed.cell.fg, /* is_fg */ true);
            let mut bg = resolve_color(indexed.cell.bg, /* is_fg */ false);
            if flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            cells[idx] = Cell {
                ch: indexed.cell.c,
                fg: rgb_to_u32(fg),
                bg: rgb_to_u32(bg),
                flags,
            };
        }

        let cursor_point = content.cursor.point;
        let cursor_row = cursor_point.line.0.max(0) as u16;
        let cursor_col = cursor_point.column.0 as u16;
        let cursor_visible = !term.mode().contains(TermMode::HIDE_CURSOR);

        Snapshot {
            cols,
            rows,
            cells,
            cursor_row,
            cursor_col,
            cursor_visible,
        }
    }
}

fn resolve_color(c: AnsiColor, is_fg: bool) -> Rgb {
    match c {
        AnsiColor::Named(n) => resolve_named(n),
        AnsiColor::Spec(rgb) => rgb,
        AnsiColor::Indexed(i) => ANSI_256[i as usize],
    }
}
```

- [ ] **Step 3d: Add write/resize/scroll on `Backend`**

```rust
use alacritty_terminal::grid::Scroll;
use std::borrow::Cow;

impl Backend {
    pub fn write_input(&self, bytes: Vec<u8>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Input(Cow::Owned(bytes)));
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let new_size = TermSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Resize(new_size.into()));
        }
        self.term.lock().resize(new_size);
        self.size = new_size;
        Ok(())
    }

    pub fn scroll(&self, delta_lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(delta_lines));
    }

    pub fn shutdown(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Shutdown);
        }
    }
}
```

- [ ] **Step 4: Rewrite `src/terminal/mod.rs` to expose the `Terminal` facade**

```rust
pub mod backend;
pub mod colors;
pub mod input;
pub mod listener;
pub mod render;
pub mod snapshot;

pub use backend::{Backend, TermSize};
pub use snapshot::{Cell, CellFlags, Snapshot};

use alacritty_terminal::event::Event as AlacEvent;
use alacritty_terminal::term::TermMode;
use anyhow::Result;
use futures::channel::mpsc::UnboundedReceiver;
use std::path::Path;

pub struct Terminal {
    backend: Backend,
    events: Option<UnboundedReceiver<AlacEvent>>,
}

impl Terminal {
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<Self> {
        let (backend, events) = Backend::spawn(cwd, cols, rows)?;
        Ok(Self {
            backend,
            events: Some(events),
        })
    }

    /// Test-only headless construction — no PTY, no shell. Drive bytes with
    /// `Terminal::feed_bytes`.
    pub fn headless(cols: u16, rows: u16) -> Self {
        let (backend, events) = Backend::headless(cols, rows);
        Self {
            backend,
            events: Some(events),
        }
    }

    pub fn take_events(&mut self) -> Option<UnboundedReceiver<AlacEvent>> {
        self.events.take()
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.backend.write_input(bytes.to_vec());
        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.backend.resize(cols, rows)
    }

    pub fn scroll(&self, delta_lines: i32) {
        self.backend.scroll(delta_lines);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.backend.snapshot()
    }

    pub fn mode(&self) -> TermMode {
        self.backend.mode()
    }

    #[cfg(test)]
    pub fn feed_bytes(&self, bytes: &[u8]) {
        self.backend.feed_bytes(bytes);
    }
}
```

- [ ] **Step 5: Delete `src/terminal/vt.rs` and `src/terminal/pty.rs`**

Run: `rm src/terminal/vt.rs src/terminal/pty.rs`

- [ ] **Step 6: Build the library — expect callers (`input.rs`, `render.rs`, `main_tabs.rs`) to fail**

Run: `cargo build --lib 2>&1 | head -60`

Expected failures, in order of importance:
- `src/terminal/input.rs` — still compiles (its surface area didn't change); proceed.
- `src/terminal/render.rs` — accesses `Cell { ch, fg, bg }` fields. Will fail on missing `flags`. We rewrite render in Task 6.
- `src/layout/main_tabs.rs` — accesses `terminal.pty.child.try_wait()`. Will fail (no `pty` field anymore). We rewrite in Task 5.

Do **not** commit yet. Task 4 lands the smoke test; Task 5 lands the gpui-side rewrite and commits.

---

## Task 4: Rewrite `tests/vt_smoke.rs` against the new headless `Terminal`

**Files:**
- Modify: `tests/vt_smoke.rs`

The test acts as the safety net for the snapshot path. It must pass before we touch the renderer or gpui glue.

- [ ] **Step 1: Replace `tests/vt_smoke.rs` with the new test bodies**

```rust
use swrm::terminal::{CellFlags, Terminal};

#[test]
fn writes_visible_characters() {
    let term = Terminal::headless(20, 5);
    term.feed_bytes(b"hello");
    let snap = term.snapshot();
    let row0: String = snap.cells[..20].iter().map(|c| c.ch).collect();
    assert!(row0.starts_with("hello"), "row0 = {:?}", row0);
}

#[test]
fn cursor_advances() {
    let term = Terminal::headless(20, 5);
    term.feed_bytes(b"abc");
    let snap = term.snapshot();
    assert_eq!(snap.cursor_row, 0);
    assert_eq!(snap.cursor_col, 3);
}

#[test]
fn sgr_bold_red_sets_cell_flags_and_fg() {
    let term = Terminal::headless(20, 5);
    // ESC[1;31m  = bold + foreground red (ANSI 1)
    term.feed_bytes(b"\x1b[1;31mX\x1b[0m");
    let snap = term.snapshot();
    let cell = snap.cell_at(0, 0).expect("cell at 0,0");
    assert_eq!(cell.ch, 'X');
    assert!(cell.flags.contains(CellFlags::BOLD), "expected BOLD, got {:?}", cell.flags);
    // ANSI palette index 1 is 0xcd0000.
    assert_eq!(cell.fg, 0x00cd_0000, "fg = 0x{:08x}", cell.fg);
}

#[test]
fn snapshot_is_size_consistent() {
    let term = Terminal::headless(40, 10);
    let snap = term.snapshot();
    assert_eq!(snap.cells.len(), 400);
    assert_eq!(snap.cols, 40);
    assert_eq!(snap.rows, 10);
}
```

- [ ] **Step 2: Run only the smoke tests**

Run: `cargo test --test vt_smoke 2>&1 | tail -30`
Expected: 4/4 pass.

Likely causes of failure and what to do:
- "no method `feed_bytes`" → the `#[cfg(test)]` gate in `mod.rs` blocked it. Remove the gate (it's a test-only path either way) or replicate this test inside `src/terminal/backend.rs` as `#[cfg(test)] mod tests`.
- SGR red is some other RGB → cross-check the palette: alacritty's default for `Named(Red)` is 0xcd0000 in the xterm palette. If the test expects 0xff0000, you imported the wrong palette index.
- "Processor::advance not found" → the headless feed path isn't using the right alacritty surface. Look at `alacritty_terminal/tests/ref/*` in the alacritty repo for an example of feeding bytes into Term in unit tests.

- [ ] **Step 3: Confirm the library compiles (callers still broken — expected)**

Run: `cargo build --lib 2>&1 | tail -30`
Expected: errors confined to `src/layout/main_tabs.rs` and `src/terminal/render.rs`. If any error originates inside `src/terminal/{backend,listener,colors,snapshot,mod}.rs`, fix it before Task 5.

- [ ] **Step 4: Do not commit yet**

The library compiles and the smoke test passes, but the full binary build still fails on `src/layout/main_tabs.rs`. Task 5 fixes that and lands the first commit.

---

## Task 5: Rewrite `TerminalTab` — event-driven gpui bridge, no tick

**Files:**
- Modify: `src/layout/main_tabs.rs`

- [ ] **Step 1: Strip the 16 ms tick from `TerminalTab::new`**

Open `src/layout/main_tabs.rs`. Find lines 22-54 (the `impl TerminalTab` block). Replace `pub fn new(...)` with this:

```rust
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
                use alacritty_terminal::event::Event::*;
                let cont = this
                    .update(cx, |this, cx| {
                        match event {
                            Wakeup | Bell | MouseCursorDirty => {
                                cx.notify();
                                true
                            }
                            Title(_) | ResetTitle => {
                                // Title plumbing comes in a follow-up; ignore for now.
                                true
                            }
                            ChildExit(_) | Exit => {
                                this.exited = true;
                                cx.notify();
                                false
                            }
                            _ => true,
                        }
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
    // … on_key stays …
}
```

- [ ] **Step 2: Add the `exited` field to `TerminalTab`**

```rust
pub struct TerminalTab {
    pub label: String,
    pub terminal: Terminal,
    pub focus: FocusHandle,
    pub exited: bool,
}
```

- [ ] **Step 3: Update `on_key` and `Render` impls — keep the OLD `input::encode` signature**

`on_key` stays nearly the same — but check `exited` first. The call to `input::encode` stays single-arg here; Task 7 swaps it to the mode-aware form:

```rust
fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
    if self.exited {
        return;
    }
    if let Some(bytes) = input::encode(event) {
        if let Err(err) = self.terminal.write_input(&bytes) {
            tracing::warn!(?err, "pty write failed");
        }
        cx.notify();
    }
}
```

Render: drop `.snapshot().ok()` (it's now infallible), and pass the new shape into `render_snapshot`. Until Task 6 rewrites the renderer, the call site is `render_snapshot(&self.terminal.snapshot())`.

```rust
impl Render for TerminalTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.terminal.snapshot();
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .bg(gpui::rgb(0x111111))
            .p_2()
            .child(render_snapshot(&snap))
    }
}
```

- [ ] **Step 4: Build — should now be GREEN**

Run: `cargo build`
Expected: clean build. `src/terminal/render.rs` still compiles because it only reads `Cell::ch` (the new `fg`/`bg`/`flags` fields are simply ignored). `src/terminal/input.rs` still compiles because its signature is unchanged. The app is now functionally working on alacritty in monochrome with the old keymap.

- [ ] **Step 5: Smoke-launch the app**

Run: `cargo run`
- A shell appears in the terminal tab.
- Type `ls` and Enter — output appears, in monochrome.
- Type ctrl-c — fresh prompt.
- Close the window cleanly.

If the shell doesn't appear or the app hangs, debug now — every later task assumes this works.

- [ ] **Step 6: Commit the atomic backend swap**

```bash
git add Cargo.toml Cargo.lock \
        src/terminal/mod.rs \
        src/terminal/snapshot.rs \
        src/terminal/backend.rs \
        src/terminal/listener.rs \
        src/terminal/colors.rs \
        src/layout/main_tabs.rs \
        tests/vt_smoke.rs
git rm src/terminal/vt.rs src/terminal/pty.rs
git commit -m "$(cat <<'EOF'
refactor: Migrate terminal backend to alacritty_terminal

Replace libghostty-vt + portable-pty with alacritty_terminal 0.26 as a
single atomic swap. The alacritty event loop owns the reader thread; gpui
re-renders are now driven by an event stream rather than a 16 ms polling
tick.

Output is still monochrome and the keymap is unchanged from the previous
baseline; per-cell ANSI colors land in the next commit, the full keymap
the one after.
EOF
)"
```

---

## Task 6: Rewrite `render.rs` — batched-run per-cell colors

**Files:**
- Modify: `src/terminal/render.rs`

Replace the row-as-joined-String renderer with one that walks each row, groups contiguous cells of equal `(fg, bg, flags)` into runs, and emits one styled `div` per run inside a row container.

- [ ] **Step 1: Replace `src/terminal/render.rs` end-to-end**

```rust
use super::{Cell, CellFlags, Snapshot};
use gpui::{IntoElement, ParentElement, Styled, div, px, rgb};

pub fn render_snapshot(snap: &Snapshot) -> impl IntoElement {
    let cols = snap.cols as usize;
    let mut col = div()
        .flex()
        .flex_col()
        .font_family("JetBrains Mono")
        .text_size(px(13.));

    for row in 0..snap.rows as usize {
        let start = row * cols;
        let row_cells = &snap.cells[start..start + cols];
        col = col.child(render_row(row_cells));
    }
    col
}

fn render_row(cells: &[Cell]) -> impl IntoElement {
    let mut row = div().flex().flex_row();
    if cells.is_empty() {
        return row;
    }

    let mut run_start = 0;
    for i in 1..=cells.len() {
        let same_run = i < cells.len()
            && cells[i].fg == cells[run_start].fg
            && cells[i].bg == cells[run_start].bg
            && cells[i].flags == cells[run_start].flags;
        if !same_run {
            let run = &cells[run_start..i];
            row = row.child(render_run(run));
            run_start = i;
        }
    }
    row
}

fn render_run(run: &[Cell]) -> impl IntoElement {
    debug_assert!(!run.is_empty());
    let head = &run[0];
    let text: String = run.iter().map(|c| c.ch).collect();

    let mut el = div()
        .text_color(rgb(head.fg))
        .bg(rgb(head.bg))
        .child(text);

    if head.flags.contains(CellFlags::BOLD) {
        el = el.font_weight(gpui::FontWeight::BOLD);
    }
    if head.flags.contains(CellFlags::ITALIC) {
        el = el.italic();
    }
    if head.flags.contains(CellFlags::UNDERLINE) {
        el = el.underline();
    }
    // STRIKEOUT, DIM, INVERSE-without-color-swap: skip for now;
    // INVERSE is already handled in Backend::snapshot by swapping fg/bg.
    el
}
```

If `gpui` doesn't expose `.font_weight()` / `.italic()` / `.underline()` directly on the styled element trait, look at `gpui_component/src/text.rs` or `gpui_component/src/label.rs` for the canonical way to apply text styling. Worst-case, set `text_color` only and ship bold/italic/underline as a follow-up — but landing them now is cheap.

- [ ] **Step 2: Build and smoke-launch**

Run: `cargo build && cargo run`
- Prompt text appears in its configured color (often green or blue).
- `ls -la` (or `ls --color=always`) prints colored directory entries.
- `printf '\033[1;31mBOLD RED\033[0m\n'` renders as bold red text.
- Background colors (e.g. `printf '\033[44mBLUE BG\033[0m\n'`) paint the full glyph cell.

If runs are visibly mis-aligned (gaps between adjacent same-style cells, or letters out of order), the issue is in `render_row`'s run-coalescing logic — re-check the equality test on `(fg, bg, flags)`.

- [ ] **Step 3: Commit**

```bash
git add src/terminal/render.rs
git commit -m "feat: Per-cell ANSI colors with batched-run renderer"
```

---

## Task 7: Port Zed's `mappings/keys.rs` shape into `src/terminal/input.rs`

**Files:**
- Modify: `src/terminal/input.rs`
- Create: `tests/input_keymap.rs`

`input::encode` now takes a `TermMode` and produces correct escape sequences for arrows under app-cursor mode, F1-F20, alt-as-meta, bracketed paste flags, Ctrl+letter, and the existing baseline. Zed's `to_esc_str(keystroke, mode, alt_is_meta) -> Option<Cow<'static, str>>` is the template.

- [ ] **Step 1: Write `tests/input_keymap.rs` first (TDD)**

```rust
use alacritty_terminal::term::TermMode;
use gpui::Keystroke;
use swrm::terminal::input::encode_from_keystroke;

fn ks(s: &str) -> Keystroke {
    Keystroke::parse(s).expect("valid keystroke")
}

#[test]
fn enter_emits_carriage_return() {
    let out = encode_from_keystroke(&ks("enter"), TermMode::empty(), false);
    assert_eq!(out.as_deref(), Some(&b"\r"[..]));
}

#[test]
fn arrow_normal_mode_emits_csi() {
    let out = encode_from_keystroke(&ks("right"), TermMode::empty(), false);
    assert_eq!(out.as_deref(), Some(&b"\x1b[C"[..]));
}

#[test]
fn arrow_app_cursor_mode_emits_ss3() {
    let out = encode_from_keystroke(&ks("right"), TermMode::APP_CURSOR, false);
    assert_eq!(out.as_deref(), Some(&b"\x1bOC"[..]));
}

#[test]
fn ctrl_c_emits_sigint_byte() {
    let out = encode_from_keystroke(&ks("ctrl-c"), TermMode::empty(), false);
    assert_eq!(out.as_deref(), Some(&b"\x03"[..]));
}

#[test]
fn alt_meta_prefixes_esc() {
    let out = encode_from_keystroke(&ks("alt-b"), TermMode::empty(), true);
    assert_eq!(out.as_deref(), Some(&b"\x1bb"[..]));
}

#[test]
fn f1_emits_ss3_p() {
    let out = encode_from_keystroke(&ks("f1"), TermMode::empty(), false);
    assert_eq!(out.as_deref(), Some(&b"\x1bOP"[..]));
}

#[test]
fn f5_emits_csi_15_tilde() {
    let out = encode_from_keystroke(&ks("f5"), TermMode::empty(), false);
    assert_eq!(out.as_deref(), Some(&b"\x1b[15~"[..]));
}

#[test]
fn plain_char_emits_itself() {
    let out = encode_from_keystroke(&ks("a"), TermMode::empty(), false);
    assert_eq!(out.as_deref(), Some(&b"a"[..]));
}
```

- [ ] **Step 2: Run the tests — they should fail with "no function `encode_from_keystroke`"**

Run: `cargo test --test input_keymap 2>&1 | tail -10`
Expected: compile error or all-fail.

- [ ] **Step 3: Implement `src/terminal/input.rs`**

```rust
use alacritty_terminal::term::TermMode;
use gpui::{KeyDownEvent, Keystroke};
use std::borrow::Cow;

/// Encode a `KeyDownEvent` for the PTY. `alt_is_meta` is the historic xterm
/// behavior of treating Alt as ESC-prefix (true on macOS by default).
pub fn encode(event: &KeyDownEvent, mode: TermMode) -> Option<Vec<u8>> {
    encode_from_keystroke(&event.keystroke, mode, true).map(Cow::into_owned)
}

/// Pure form, exposed for tests.
pub fn encode_from_keystroke(
    keystroke: &Keystroke,
    mode: TermMode,
    alt_is_meta: bool,
) -> Option<Cow<'static, [u8]>> {
    let key = keystroke.key.as_str();
    let m = &keystroke.modifiers;

    // Ctrl+letter — emit a single control byte.
    if m.control && !m.platform && key.len() == 1 {
        if let Some(ch) = key.chars().next() {
            if ch.is_ascii_alphabetic() {
                return Some(Cow::Owned(vec![ch.to_ascii_lowercase() as u8 - b'a' + 1]));
            }
            // Ctrl+space → NUL, Ctrl+\ → 0x1c, Ctrl+] → 0x1d, Ctrl+_ → 0x1f
            return match ch {
                ' ' | '@' => Some(Cow::Borrowed(b"\x00")),
                '\\' => Some(Cow::Borrowed(b"\x1c")),
                ']' => Some(Cow::Borrowed(b"\x1d")),
                '^' => Some(Cow::Borrowed(b"\x1e")),
                '_' | '?' => Some(Cow::Borrowed(b"\x1f")),
                _ => None,
            };
        }
    }

    // Named keys (arrows / function keys / nav).
    let named = match key {
        "enter" => Some(Cow::Borrowed(b"\r" as &[u8])),
        "tab" => Some(Cow::Borrowed(b"\t" as &[u8])),
        "backspace" => Some(Cow::Borrowed(b"\x7f" as &[u8])),
        "escape" => Some(Cow::Borrowed(b"\x1b" as &[u8])),
        "left"  => Some(arrow(mode, b'D')),
        "right" => Some(arrow(mode, b'C')),
        "up"    => Some(arrow(mode, b'A')),
        "down"  => Some(arrow(mode, b'B')),
        "home"  => Some(arrow(mode, b'H')),
        "end"   => Some(arrow(mode, b'F')),
        "pageup"   => Some(Cow::Borrowed(b"\x1b[5~" as &[u8])),
        "pagedown" => Some(Cow::Borrowed(b"\x1b[6~" as &[u8])),
        "insert" => Some(Cow::Borrowed(b"\x1b[2~" as &[u8])),
        "delete" => Some(Cow::Borrowed(b"\x1b[3~" as &[u8])),
        "f1"  => Some(Cow::Borrowed(b"\x1bOP" as &[u8])),
        "f2"  => Some(Cow::Borrowed(b"\x1bOQ" as &[u8])),
        "f3"  => Some(Cow::Borrowed(b"\x1bOR" as &[u8])),
        "f4"  => Some(Cow::Borrowed(b"\x1bOS" as &[u8])),
        "f5"  => Some(Cow::Borrowed(b"\x1b[15~" as &[u8])),
        "f6"  => Some(Cow::Borrowed(b"\x1b[17~" as &[u8])),
        "f7"  => Some(Cow::Borrowed(b"\x1b[18~" as &[u8])),
        "f8"  => Some(Cow::Borrowed(b"\x1b[19~" as &[u8])),
        "f9"  => Some(Cow::Borrowed(b"\x1b[20~" as &[u8])),
        "f10" => Some(Cow::Borrowed(b"\x1b[21~" as &[u8])),
        "f11" => Some(Cow::Borrowed(b"\x1b[23~" as &[u8])),
        "f12" => Some(Cow::Borrowed(b"\x1b[24~" as &[u8])),
        _ => None,
    };
    if let Some(seq) = named {
        return Some(maybe_meta(seq, m.alt, alt_is_meta));
    }

    // Single-char keys.
    let text = keystroke.key_char.clone().or_else(|| {
        if key.chars().count() == 1 {
            Some(key.to_string())
        } else {
            None
        }
    })?;
    let bytes = text.into_bytes();
    Some(maybe_meta(Cow::Owned(bytes), m.alt, alt_is_meta))
}

fn arrow(mode: TermMode, final_byte: u8) -> Cow<'static, [u8]> {
    // App-cursor: ESC O <X>. Otherwise: ESC [ <X>.
    if mode.contains(TermMode::APP_CURSOR) {
        Cow::Owned(vec![0x1b, b'O', final_byte])
    } else {
        Cow::Owned(vec![0x1b, b'[', final_byte])
    }
}

fn maybe_meta(seq: Cow<'static, [u8]>, alt: bool, alt_is_meta: bool) -> Cow<'static, [u8]> {
    if alt && alt_is_meta {
        let mut out = Vec::with_capacity(seq.len() + 1);
        out.push(0x1b);
        out.extend_from_slice(&seq);
        Cow::Owned(out)
    } else {
        seq
    }
}
```

- [ ] **Step 4: Re-run the keymap tests — they should pass**

Run: `cargo test --test input_keymap 2>&1 | tail -10`
Expected: 8/8 pass.

If `Keystroke::parse` doesn't recognize `"alt-b"` or `"ctrl-c"` syntax, swap to the syntax gpui actually accepts (likely `"alt+b"` / `"ctrl+c"`). Adjust the test strings — don't tweak the production code to compensate.

- [ ] **Step 5: Update `TerminalTab::on_key` to pass `TermMode`**

`input::encode` now needs the current `TermMode`. Open `src/layout/main_tabs.rs` and change the call site:

```rust
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
```

- [ ] **Step 6: Library build + all tests green**

Run: `cargo build`
Expected: clean build.

Run: `cargo test 2>&1 | tail -20`
Expected: all green — `workspace_persistence`, `git_status`, `vt_smoke`, `input_keymap`.

- [ ] **Step 7: Smoke-launch and exercise the new bindings**

Run: `cargo run`
- Open `vim` or `nano`. Arrow keys move the cursor correctly (app-cursor mode triggers).
- Press F5 inside a shell — confirm the escape sequence is what the shell expected (most shells beep or do nothing visible; the absence of a stuck-input symptom is the signal).
- Hold alt and press `b` in an interactive shell with readline — cursor jumps a word back (alt-meta works).

- [ ] **Step 8: Commit**

```bash
git add src/terminal/input.rs src/layout/main_tabs.rs tests/input_keymap.rs
git commit -m "feat: Port Zed-shaped keymap with TermMode awareness"
```

---

## Task 8: Mouse-wheel scrollback

**Files:**
- Modify: `src/layout/main_tabs.rs`

- [ ] **Step 1: Add `on_scroll_wheel` to `TerminalTab::render`**

In the `Render` impl of `TerminalTab`, add a scroll handler to the root `div`. gpui exposes scroll deltas on the `ScrollWheelEvent`. Convert the vertical pixel delta into a line delta (positive = scroll *up* into scrollback; alacritty's `Scroll::Delta(positive)` scrolls history up onto screen):

```rust
use gpui::ScrollWheelEvent;

impl Render for TerminalTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.terminal.snapshot();
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .size_full()
            .bg(gpui::rgb(0x111111))
            .p_2()
            .child(render_snapshot(&snap))
    }
}

impl TerminalTab {
    fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // Approximate line height — keep consistent with text_size in renderer.
        let line_height_px = 18.0;
        let delta_y = event.delta.pixel_delta(line_height_px.into()).y.0;
        let lines = (delta_y / line_height_px) as i32;
        if lines == 0 {
            return;
        }
        self.terminal.scroll(lines);
        cx.notify();
    }
}
```

The exact accessor on `ScrollWheelEvent` may differ — check `gpui::ScrollDelta` shape on the pinned gpui. Look at `gpui-component/src/scroll.rs` or other call sites in this repo for prior art.

- [ ] **Step 2: Build + run the app**

Run: `cargo build && cargo run`
Manually verify in the running app:
- Open a terminal tab.
- `seq 1 200` (or similar) to produce a long output.
- Mouse-wheel up — older lines come into view.
- Mouse-wheel down — returns to the prompt.
- Type a key while scrolled back — alacritty automatically jumps to the bottom (this is alacritty's default; verify it happens).

If the scrollback is "sticky" (you scroll up and the prompt vanishes off the bottom), that's correct alacritty behavior. If it doesn't auto-jump on input, that's a gap to fix in a follow-up — note it in the PR description.

- [ ] **Step 3: Commit**

```bash
git add src/layout/main_tabs.rs
git commit -m "feat: Wire mouse-wheel scrollback to alacritty Term::scroll_display"
```

---

## Task 9: Remove the macOS first-build setup section from CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Delete the `### First-build setup (macOS)` block**

Open `CLAUDE.md`. Delete the entire `### First-build setup (macOS)` subsection — heading, body paragraph, and code block. Keep the `## Common commands` table immediately above it.

The deleted block is exactly:

````markdown
### First-build setup (macOS)

`libghostty-vt` invokes `zig build` from a build script and pins **Zig 0.15.2**. The system default is usually newer (0.16+), which fails. Install `zig@0.15` via Homebrew and prepend it for fresh/clean builds:

```bash
brew install zig@0.15
PATH="/opt/homebrew/opt/zig@0.15/bin:$PATH" cargo build
```

Incremental builds reuse the cached build-script output and don't need the override; only do this for the first build or after `cargo clean`.
````

- [ ] **Step 2: Spot-check the rest of CLAUDE.md for `libghostty-vt` mentions**

Run: `grep -n -i 'libghostty\|portable-pty\|zig' CLAUDE.md`
Expected: only matches inside historical / unrelated content (likely none). If any other paragraph still references libghostty, rewrite or delete that paragraph too.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: Drop libghostty-vt / Zig 0.15.2 setup notes from CLAUDE.md"
```

---

## Task 10: Final verification

**Files:** none — verification only.

- [ ] **Step 1: Clean build from scratch — confirm no Zig needed**

Run: `cargo clean && cargo build`
Expected: completes with no `zig` invocation in the build script output. No errors. `libghostty-vt` not in `Cargo.lock`.

Verify with: `grep -i 'libghostty\|portable-pty\|zig' Cargo.lock`
Expected: no matches.

- [ ] **Step 2: Run the full test suite**

Run: `cargo test`
Expected: all tests green — `workspace_persistence`, `git_status`, `vt_smoke`, `input_keymap`.

- [ ] **Step 3: Manual smoke in the running app**

Run: `cargo run`

Check in order:
1. App launches; the left sidebar lists workspaces, the main panel shows a terminal tab.
2. Run `ls -la` — colors render (directory entries are coloured if the shell has `ls --color` or equivalent).
3. Run `vim` (or `nano` / `less`). Cursor moves to the right column, alt-keys work, arrows work.
4. In `vim`, press `:q` and Enter — returns to shell. Type `^C` (ctrl-c) — fresh prompt appears.
5. Run `seq 1 500` — output scrolls past. Mouse-wheel up — history visible. Mouse-wheel down or press a key — return to prompt.
6. Open a second tab via the `+` button. Confirm each tab has independent shell state.
7. Switch workspaces via the left sidebar. Confirm each workspace keeps its own tab set.

If any of these regress vs. pre-migration baseline (other than features explicitly added — colors, scrollback), fix before merging.

- [ ] **Step 4: `cargo fmt`**

Run: `cargo fmt`
Expected: no diff. If there is one, commit it.

```bash
git diff --stat
# If non-empty:
git add -u && git commit -m "chore: cargo fmt"
```

- [ ] **Step 5: Done**

Branch is ready for review. Open the PR with a description that calls out:
- Backend swap (libghostty-vt + portable-pty → alacritty_terminal 0.26).
- macOS first-build pain removed; Zig 0.15.2 no longer required.
- Per-cell ANSI colors, bold/italic/underline, batched-run rendering.
- Full keymap incl. app-cursor/app-keypad, F1-F12, alt-meta, bracketed paste.
- Mouse-wheel scrollback.
- **Deferred:** selection + clipboard, mouse reporting, OSC 8 hyperlinks, IME, resize-to-bounds. File follow-up issues.

---

## Risk register

These are the parts most likely to surprise the implementer:

1. **`alacritty_terminal` exact API drift** — `Term::new` / `Notifier` / `Processor` may have differently-named methods in 0.26 than the research summary suggests. Always cross-check docs.rs. Zed's `crates/terminal/src/terminal.rs` is the canonical embedder reference; copy its shape if in doubt.
2. **`Color::Named` enum exhaustiveness** — `colors::resolve_named` must handle every `NamedColor` variant. A non-exhaustive match will fail to compile, which is the correct outcome — fix the match arms, don't add a catch-all that hides drift.
3. **gpui scroll-wheel API** — `ScrollWheelEvent::delta` may be a different shape; `ScrollDelta::Lines` vs `ScrollDelta::Pixels` may need branching. Check the gpui pinned rev's `gpui/src/platform/mouse.rs` or similar.
4. **`bitflags` version** — if `bitflags` isn't already in the compile path with the right derive macros, add `bitflags = "2"` to `Cargo.toml`.
5. **Headless `feed_bytes` path** — if alacritty 0.26 doesn't expose a public byte→Term plumbing for tests, the headless smoke test will need to drive `vte::Parser` + a `vte::Perform` impl on the Term wrapper. This is the most likely place to lose 30-60 minutes. Look at `alacritty_terminal/src/term/tests.rs` upstream for the pattern.
6. **macOS PTY child detection** — `Event::ChildExit(ExitStatus)` arrives after the child reaper does its work; small race with shutdown-on-window-close. The current code didn't handle this either; not a regression, just a known gap.

---

## Open follow-up backlog (out-of-scope but tracked)

Land each as a separate plan + PR after the migration:

- **Selection + clipboard copy** — `term.selection`, `HighlightedRange`, `cx.write_to_clipboard`.
- **Mouse reporting to the app** — translate gpui mouse events into SGR / normal mouse encoding, gated on `TermMode::MOUSE_REPORT_*`.
- **OSC 8 hyperlinks** — `Cell::hyperlink()` + ctrl-hover underline + click handling.
- **IME composition** — gpui input handler integration; needed for CJK / emoji-via-IME.
- **Resize-to-element-bounds** — measure the rendered cell size on first paint, then call `Terminal::resize(cols, rows)` when the container resizes. The 80×24 hardcode is currently a real usability cap.
- **cwd tracking** — port Zed's `pty_info.rs` (sysinfo + `tcgetpgrp`) so the right sidebar's "git status" view follows the shell's cwd.
- **Bell handling** — currently the `Bell` event is silently ignored; decide on visual flash, sound, or both.
