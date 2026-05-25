use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};

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
        // `--data-binary @-` forwards Claude's hook JSON (piped on stdin)
        // straight into the POST body so swrm can extract tool activity
        // for the sidebar's activity line. `--max-time 1` keeps a slow
        // hook from blocking Claude. `-s -o /dev/null` quiets curl.
        format!(
            "curl -s --max-time 1 -o /dev/null -X POST --data-binary @- {}",
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

/// Returns true iff `substitute_placeholder` would change `command` —
/// i.e. the command contains `${CLAUDE_SETTINGS}` or a word-boundary
/// `$CLAUDE_SETTINGS` occurrence (not part of a longer identifier like
/// `$CLAUDE_SETTINGS_DIR`).
pub fn has_placeholder(command: &str) -> bool {
    command.contains("${CLAUDE_SETTINGS}") || has_bare_placeholder(command)
}

/// Replace every occurrence of `$CLAUDE_SETTINGS` and `${CLAUDE_SETTINGS}`
/// in `command` with `path`. Bare form requires a word boundary on the
/// right (next char is non-`[A-Za-z0-9_]`, or end-of-string), so a longer
/// variable like `$CLAUDE_SETTINGS_DIR` is left untouched.
pub fn substitute_placeholder(command: &str, path: &str) -> String {
    let braced = command.replace("${CLAUDE_SETTINGS}", path);
    replace_bare_placeholder(&braced, path)
}

/// `${TMPDIR}/swrm-<pid>/` — the per-process directory we write tab settings
/// files into. PID-scoped so multiple swrm processes don't collide and so
/// the parent dir can be wiped on shutdown without affecting other tools.
/// Best-effort cleanup on app exit is not implemented; macOS wipes /tmp on
/// reboot which is sufficient for the MVP.
pub fn temp_settings_dir() -> PathBuf {
    std::env::temp_dir().join(format!("swrm-{}", std::process::id()))
}

/// Write `json` to `path`, creating the parent directory if necessary.
/// Overwrites any existing file.
pub fn write_settings_file(path: &Path, json: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create settings parent {parent:?}"))?;
    }
    std::fs::write(path, json).with_context(|| format!("write settings file {path:?}"))?;
    Ok(())
}

fn has_bare_placeholder(command: &str) -> bool {
    let needle = "$CLAUDE_SETTINGS";
    let mut start = 0;
    while let Some(pos) = command[start..].find(needle) {
        let abs = start + pos;
        let after = abs + needle.len();
        if !next_is_word_char(command, after) {
            return true;
        }
        start = after;
    }
    false
}

fn replace_bare_placeholder(command: &str, path: &str) -> String {
    let needle = "$CLAUDE_SETTINGS";
    let mut out = String::with_capacity(command.len());
    let mut cursor = 0;
    while let Some(pos) = command[cursor..].find(needle) {
        let abs = cursor + pos;
        let after = abs + needle.len();
        out.push_str(&command[cursor..abs]);
        if next_is_word_char(command, after) {
            // Part of a longer identifier (e.g. $CLAUDE_SETTINGS_DIR) —
            // keep the literal.
            out.push_str(needle);
        } else {
            out.push_str(path);
        }
        cursor = after;
    }
    out.push_str(&command[cursor..]);
    out
}

fn next_is_word_char(s: &str, byte_idx: usize) -> bool {
    s.as_bytes()
        .get(byte_idx)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
}
