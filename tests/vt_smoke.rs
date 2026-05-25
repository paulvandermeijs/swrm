use std::time::{Duration, Instant};
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
    assert!(
        cell.flags.contains(CellFlags::BOLD),
        "expected BOLD, got {:?}",
        cell.flags
    );
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

/// Documents the underlying vte behaviour we work around in `Backend::spawn`:
/// the parser treats `ESC k` as an unknown single-byte escape, drops back to
/// ground, and prints the following bytes as literal text. We avoid hitting
/// this by forcing `TERM=xterm-256color` on the child shell, so oh-my-zsh uses
/// OSC title sequences (which vte handles) rather than the screen/tmux form.
#[test]
fn screen_title_sequence_leaks_into_grid() {
    let term = Terminal::headless(40, 5);
    term.feed_bytes(b"\x1bkls\x1b\\Cargo.lock");
    let snap = term.snapshot();
    let row0: String = snap.cells[..40].iter().map(|c| c.ch).collect();
    assert!(
        row0.trim_end().starts_with("lsCargo.lock"),
        "expected vte to leak the title payload; got {:?}",
        row0.trim_end()
    );
}

/// Pins the env override that fixes the leak above end-to-end: a child shell
/// spawned via `Terminal::spawn` must see `TERM=xterm-256color` regardless of
/// the parent's `TERM` (which can be `tmux-256color`, `screen`, etc. when swrm
/// is launched from a multiplexer).
#[test]
fn child_shell_sees_xterm_256color_term() {
    // Override the parent's TERM to something that would trigger the bug, so
    // we're actually proving we override and not just passing inheritance.
    // SAFETY: tests run single-threaded here w.r.t. this var; cargo test is
    // multi-threaded by default but other tests don't read TERM.
    unsafe { std::env::set_var("TERM", "tmux-256color") };

    let cwd = std::env::temp_dir();
    let mut term = Terminal::spawn(&cwd, 80, 6).expect("spawn child shell");
    // Drop the events receiver so the channel doesn't fill up.
    let _ = term.take_events();

    // Use /bin/sh-compatible syntax — works in zsh, bash, sh.
    term.write_input(b"printf 'TERM=[%s]\\n' \"$TERM\"\rexit\r")
        .expect("write to pty");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snap = term.snapshot();
        let grid: String = snap.cells.iter().map(|c| c.ch).collect();
        if grid.contains("TERM=[xterm-256color]") {
            return;
        }
        if grid.contains("TERM=[tmux-256color]") {
            panic!("child inherited parent TERM instead of our override: {grid:?}");
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for child shell output; grid = {grid:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
