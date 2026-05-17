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

    // Named keys.
    let named = match key {
        "enter" => Some(Cow::Borrowed(b"\r" as &[u8])),
        "tab" => Some(Cow::Borrowed(b"\t" as &[u8])),
        "backspace" => Some(Cow::Borrowed(b"\x7f" as &[u8])),
        "escape" => Some(Cow::Borrowed(b"\x1b" as &[u8])),
        "left" => Some(arrow(mode, b'D')),
        "right" => Some(arrow(mode, b'C')),
        "up" => Some(arrow(mode, b'A')),
        "down" => Some(arrow(mode, b'B')),
        "home" => Some(arrow(mode, b'H')),
        "end" => Some(arrow(mode, b'F')),
        "pageup" => Some(Cow::Borrowed(b"\x1b[5~" as &[u8])),
        "pagedown" => Some(Cow::Borrowed(b"\x1b[6~" as &[u8])),
        "insert" => Some(Cow::Borrowed(b"\x1b[2~" as &[u8])),
        "delete" => Some(Cow::Borrowed(b"\x1b[3~" as &[u8])),
        "f1" => Some(Cow::Borrowed(b"\x1bOP" as &[u8])),
        "f2" => Some(Cow::Borrowed(b"\x1bOQ" as &[u8])),
        "f3" => Some(Cow::Borrowed(b"\x1bOR" as &[u8])),
        "f4" => Some(Cow::Borrowed(b"\x1bOS" as &[u8])),
        "f5" => Some(Cow::Borrowed(b"\x1b[15~" as &[u8])),
        "f6" => Some(Cow::Borrowed(b"\x1b[17~" as &[u8])),
        "f7" => Some(Cow::Borrowed(b"\x1b[18~" as &[u8])),
        "f8" => Some(Cow::Borrowed(b"\x1b[19~" as &[u8])),
        "f9" => Some(Cow::Borrowed(b"\x1b[20~" as &[u8])),
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
