# Terminal backend research

Date: 2026-05-17
Status: research, not yet a plan

This document captures research into terminal-emulation backends for `swrm`.
It is intended as input for a follow-up implementation plan. The recommended
direction at the end is a suggestion, not a decision.

## 1. What swrm has today

The terminal stack is small and explicitly skeletal.

- **PTY** — `src/terminal/pty.rs` spawns `$SHELL` via `portable-pty` 0.9. A
  dedicated reader thread fan-outs `Vec<u8>` chunks onto an `mpsc::channel`.
- **VT** — `src/terminal/vt.rs` wraps `libghostty-vt`'s `Terminal` +
  `RenderState` + reusable `RowIterator` / `CellIterator`. Flattens the grid
  into a `Snapshot` of `{ ch: char, fg: u32, bg: u32 }` cells plus cursor
  row/col.
- **Drive loop** — `TerminalTab::new` in `src/layout/main_tabs.rs:25-48`
  polls every 16 ms, drains the channel into the VT, calls `cx.notify()` if
  anything changed.
- **Render** — `src/terminal/render.rs` builds one `div` per row containing
  the joined `char`s. No per-cell colors, no bold/italic/underline, no
  cursor glyph, no selection, no scrollback UI.
- **Input** — `src/terminal/input.rs` is 35 lines: arrows, enter, backspace,
  tab, escape, Ctrl+letter. No alt/meta, no F-keys, no app-cursor / keypad
  modes, no mouse, no bracketed paste, no IME.
- **Build pain** — `libghostty-vt` invokes `zig build` from a build script
  and pins **Zig 0.15.2**. macOS users need `zig@0.15` from Homebrew for the
  first build; documented in `CLAUDE.md` under "First-build setup (macOS)".

The VT itself can technically handle most escape sequences (libghostty's
parser is solid), but the surrounding swrm code only exposes ~5% of what a
real terminal would surface. The Zig pin is a real distribution tax.

## 2. How Zed does it (same gpui runtime, different backend)

Zed pins **`alacritty_terminal` from a Zed-maintained fork** at rev
`9d9640d4` in its workspace `Cargo.toml`. Key files in the Zed repo:

- `crates/terminal/src/terminal.rs:455-750` — `Terminal` struct,
  `tty::new(...)` plus alacritty `EventLoop::spawn()`. One OS thread per
  terminal parses bytes directly into `Term` behind a `FairMutex`.
- `crates/terminal/src/terminal.rs:1100-1550` — `process_terminal_event`,
  `InternalEvent::Resize` (does PTY winsize + `term.resize()` in one go),
  scroll, selection plumbing.
- `crates/terminal_view/src/terminal_element.rs:996-1100, 1468-1545` —
  hand-rolled gpui `Element` impl. Walks `RenderableContent` and builds
  **batched same-style `TextRun`s** (not per-cell `div`s), then `shape_line`
  + `ShapedLine::paint`. Cursor, selection (`HighlightedRange::paint`), and
  hyperlink underline are layered in.
- `crates/terminal/src/mappings/keys.rs` (~424 lines) — keystroke → escape
  sequence translation with app-cursor branching, F1-F20, alt/meta,
  bracketed paste. **No CSI-u / Kitty keyboard protocol.**
- `crates/terminal/src/mappings/mouse.rs:228-285` — SGR + normal mouse
  reporting.
- `crates/terminal/src/terminal_hyperlinks.rs` (~2350 lines, mostly tests) —
  OSC 8 hyperlink expansion + path/URL regex fallback.
- `crates/terminal/src/pty_info.rs` — cwd / foreground-process tracking via
  **sysinfo polling + `tcgetpgrp`**, not via shell-integration scripts (Zed
  does not inject any).

Architecturally important: Zed has **no 16 ms tick**. The alacritty thread
parses and emits `AlacTermEvent`s; `Event::Wakeup` is observed via
`cx.subscribe_in`, which calls `cx.notify()`. Pure event-driven re-render.

## 3. Backend landscape (scored 0-3)

| Backend | Maint. | Lic. | API | OSC 8 | Mouse/BP | Kitty gfx | Sixel | Kitty kbd | Scroll+search | Build cost | Embedders | Total |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **alacritty_terminal** | 3 | 3 | 3 | 3 | 3 | 0 | 0 | 0 | 3 | 3 | 3 | **24** |
| libghostty-vt *(today)* | 2 | 3 | 2 | 2 | 3 | 3 | 0 | 2 | 2 | 0 | 0 | **19** |
| **wezterm-term** | 2 | 3 | 2 | 3 | 3 | 3 | 3 | 3 | 3 | 1 | 1 | **27** |
| vte + custom grid | 3 | 3 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 3 | **13** |
| avt (asciinema) | 3 | 3 | 3 | 0 | 0 | 0 | 0 | 0 | 1 | 3 | 1 | **14** |
| copa (rio) | 3 | 3 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 1 | **11** |

Notes per backend:

- **alacritty_terminal** — Apache-2.0. Most-deployed embeddable Rust VT.
  Full SGR, 24-bit color, mouse, bracketed paste, OSC 8 hyperlinks stored
  on `Cell::hyperlink()`, OSC 7 cwd, OSC 52 clipboard via `EventListener`,
  built-in regex search (`term::search`), built-in `Selection`, mature
  scrollback. **No** Sixel, Kitty graphics, or Kitty keyboard. Zed runs a
  fork because upstream evolves slowly for embedders. Used by Alacritty,
  Zed, historically Warp, several IDE plugins.
- **libghostty-vt** *(current)* — MIT bindings (Uzaaft/libghostty-rs) over
  Ghostty's Zig VT. **Zero published releases**, single maintainer,
  requires Zig 0.15.x, ships no prebuilt artifacts, swrm is the only known
  production embedder. Ghostty's VT itself is excellent (Kitty graphics,
  Kitty keyboard, OSC 8, synchronized rendering, light/dark notifications)
  but the binding's surface may lag the upstream feature set.
- **wezterm-term** — MIT, not on crates.io (git dep only, pinned rev).
  Technical winner: Sixel + Kitty graphics + Kitty keyboard + bidi via
  `wezterm-bidi` + first-class OSC 8 + strongest unicode pipeline. Cost:
  ~10 `wezterm-*` sub-crates compiled in, tight coupling to WezTerm
  internals, velocity slowed post-maintainer-handoff.
- **vte** — parser only. Building a grid/scrollback/selection/hyperlinks
  layer on top is months of work for feature parity. Useful as a building
  block, not a backend.
- **avt** (asciinema) — Apache-2.0, very active, pure-Rust, no deps. Built
  for playback. No mouse encoding, no OSC 8, no graphics. Not suitable for
  hosting a live shell.
- **copa** (rio fork of vte) — same shape as vte. Coupled to Rio
  internals. No grid layer. Not a meaningful alternative.
- **termwiz / zellij / tmt** — not viable embeddable backends for our
  shape; see survey notes.

## 4. Comparison: swrm today vs Zed/alacritty

| # | Dimension | swrm today | Zed (alacritty) |
| --- | --- | --- | --- |
| 1 | Backend | `libghostty-vt` (Zig static lib) | `alacritty_terminal` (Zed fork) |
| 2 | PTY | `portable-pty` + manual reader thread | alacritty `tty` + alacritty `EventLoop` thread |
| 3 | Bridge | 16 ms polling tick | Event-driven `AlacTermEvent` → `cx.notify()` |
| 4 | Render | One `div` per row, no colors/attrs | Batched `TextRun`s via `shape_line`; full ANSI / themed |
| 5 | Input | ~10 keys | Full keymap incl. app-cursor, bracketed paste, IME |
| 6 | Scrollback / selection | None | Native, with copy + regex search-in-terminal |
| 7 | Resize | Wrapper only | One event resizes Term + PTY winsize atomically |
| 8 | Hyperlinks | None | OSC 8 + path/URL regex + ctrl-hover tooltip |
| 9 | cwd tracking | None | sysinfo + `tcgetpgrp` polling |
| 10 | Threading | Main + reader | Per-terminal alacritty thread + main |
| 11 | Known limits | Effectively all of the above | Key-mapping edge cases, no CSI-u, occasional hyperlink misses |

## 5. Recommendation

**Migrate to `alacritty_terminal`.** It is the boring, well-trodden choice
and — critically — the one *already validated against gpui* by Zed.

Cheap wins from the switch:

1. **Delete the Zig 0.15.2 build requirement** and the macOS first-build
   instructions in `CLAUDE.md`.
2. **Drop in scrollback, mouse selection, copy, regex search, OSC 8,
   bracketed paste, app-cursor / app-keypad modes, full SGR colors**
   essentially for free — those are alacritty types, not things we
   re-implement.
3. **A real reference implementation exists** for the gpui rendering and
   key-mapping layers: copy the shape of `terminal.rs` +
   `terminal_element.rs` + `mappings/keys.rs` (~1500 lines, much of it
   portable as-is).
4. **Active multi-embedder ecosystem.** When alacritty fixes a parser bug
   or adds a sequence, we get it.

What we give up: Ghostty's nicer parser (more spec-compliant DECRQM,
wide-char/grapheme handling), Kitty graphics, and Kitty keyboard protocol.
None of those are load-bearing for a worktree-management app whose
terminal exists to run `git` / `cargo` / shells.

If we want to keep libghostty-vt anyway (because Ghostty's VT compliance
matters), the only way to make it tolerable is to vendor prebuilt
`libghostty-vt.a` static libs per platform, signed for macOS, and rebuild
them when Ghostty drifts — a release-engineering project that competes
with shipping the app.

`wezterm-term` is worth a second look only if inline images (Sixel / Kitty
graphics) or full Kitty keyboard support land on the swrm roadmap.

## 6. Suggested migration shape (input for the plan)

Incremental and reversible:

1. Introduce a `terminal::vt::Backend` trait around the methods
   `VtWrapper` already has: `feed(&[u8])`, `resize(cols, rows)`,
   `snapshot() -> Snapshot`, plus a way to subscribe to wakeups (replaces
   the polling tick).
2. Rewrite `tests/vt_smoke.rs` against the trait, not the concrete type.
3. Add an `alacritty` cargo feature with a new `Backend` impl wrapping
   `alacritty_terminal::Term` + its `EventLoop`. Keep `libghostty` as the
   other impl during the transition.
4. Extend `Snapshot` (or replace it) so cells carry full ANSI / SGR state
   (24-bit fg/bg, bold/italic/underline/strike/inverse/dim). Port the
   renderer in `src/terminal/render.rs` to read per-cell colors and emit
   batched `TextRun`s the way `terminal_element.rs` does.
5. Port the keymap. Zed's `mappings/keys.rs` is the template — app-cursor
   branching, F-keys, alt/meta, bracketed paste, IME via gpui's input
   handler.
6. Wire selection + copy + scrollback.
7. Flip the default to `alacritty`, ship a release, delete the libghostty
   path next release. The Zig build script and the `libghostty-vt`
   dependency disappear with it. Delete the "First-build setup (macOS)"
   section of `CLAUDE.md`.

## 7. Open questions for the plan

- Do we want the migration to be feature-flagged through one or two
  releases, or do we cut over in a single commit?
- Do we vendor a fork of `alacritty_terminal` (like Zed does at rev
  `9d9640d4`) or pin upstream crates.io? The fork exists because upstream
  is slow to accept embedder-only changes — but we may not need any of
  those changes initially.
- Do we keep `portable-pty` or switch to `alacritty_terminal::tty`? Zed
  uses the latter and gets the alacritty event-loop "for free"; keeping
  `portable-pty` means re-implementing the event pump.
- Scope of the first cut: do we ship colors + scrollback + selection in
  one PR, or land each as a separate PR against the `Backend` trait?
- IME / mouse / hyperlinks — phase 2, or part of the initial migration?

## 8. Key files in this repo

- `Cargo.toml` — `libghostty-vt` dep lives here.
- `src/terminal/mod.rs` — `Terminal` glue.
- `src/terminal/vt.rs` — `VtWrapper`, `Snapshot`, `Cell`. The trait should
  carve out of this.
- `src/terminal/pty.rs` — `PtySession` via `portable-pty`.
- `src/terminal/input.rs` — minimal keymap; replace per Zed
  `mappings/keys.rs`.
- `src/terminal/render.rs` — `render_snapshot`; rewrite to per-cell colors
  + batched runs.
- `src/layout/main_tabs.rs:25-48` — the 16 ms tick; replace with event
  subscription.
- `tests/vt_smoke.rs` — make trait-shaped first.
- `CLAUDE.md` — "First-build setup (macOS)" section to delete on cutover.

## 9. Sources

- github.com/zed-industries/zed — `crates/terminal/`, `crates/terminal_view/`
- github.com/zed-industries/alacritty (Zed's fork, rev `9d9640d4`)
- github.com/alacritty/alacritty — `alacritty_terminal` v0.25.x line
- github.com/Uzaaft/libghostty-rs — current dep
- github.com/wezterm/wezterm — `term/`
- github.com/asciinema/avt
- github.com/raphamorim/rio — `copa`, `teletypewriter`
- github.com/alacritty/vte
