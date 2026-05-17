use gpui::KeyDownEvent;

pub fn encode(event: &KeyDownEvent) -> Option<Vec<u8>> {
    let keystroke = &event.keystroke;
    if keystroke.modifiers.control {
        if let Some(ch) = keystroke.key.chars().next() {
            if ch.is_ascii_alphabetic() {
                let lower = ch.to_ascii_lowercase() as u8;
                return Some(vec![lower - b'a' + 1]);
            }
        }
    }
    match keystroke.key.as_str() {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(b"\x7f".to_vec()),
        "tab" => Some(b"\t".to_vec()),
        "escape" => Some(b"\x1b".to_vec()),
        "left" => Some(b"\x1b[D".to_vec()),
        "right" => Some(b"\x1b[C".to_vec()),
        "up" => Some(b"\x1b[A".to_vec()),
        "down" => Some(b"\x1b[B".to_vec()),
        _ => keystroke.key_char.clone().or_else(|| {
            let k = &keystroke.key;
            if k.chars().count() == 1 { Some(k.clone()) } else { None }
        })
        .map(|s| s.into_bytes()),
    }
}
