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
        // --max-time 1: a slow hook must not block Claude. -s: quiet.
        // -o /dev/null: swallow the empty body. -X POST: no body needed.
        format!("curl -s --max-time 1 -o /dev/null -X POST {}", url(status))
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

/// Replace every occurrence of `$CLAUDE_SETTINGS` and `${CLAUDE_SETTINGS}`
/// in `command` with `path`. Bare and braced forms both work; the command
/// is passed to `$SHELL -c` so we substitute on the swrm side rather than
/// relying on the spawned shell's variable expansion.
pub fn substitute_placeholder(command: &str, path: &str) -> String {
    command
        .replace("${CLAUDE_SETTINGS}", path)
        .replace("$CLAUDE_SETTINGS", path)
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
