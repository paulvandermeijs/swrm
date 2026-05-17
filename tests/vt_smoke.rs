use swrm::terminal::VtWrapper;

#[test]
fn writes_visible_characters() {
    let mut vt = VtWrapper::new(20, 5).unwrap();
    vt.feed(b"hello");
    let snap = vt.snapshot().unwrap();
    let row0: String = snap.cells[..20].iter().map(|c| c.ch).collect();
    assert!(row0.starts_with("hello"));
}

#[test]
fn cursor_advances() {
    let mut vt = VtWrapper::new(20, 5).unwrap();
    vt.feed(b"abc");
    let snap = vt.snapshot().unwrap();
    assert_eq!(snap.cursor_row, 0);
    assert_eq!(snap.cursor_col, 3);
}
