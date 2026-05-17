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
