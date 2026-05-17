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
