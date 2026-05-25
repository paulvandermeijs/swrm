use std::path::PathBuf;
use swrm::agent_status::server::parse_event_path;
use swrm::agent_status::store::aggregate_status;
use swrm::agent_status::{
    AgentStatus, HookEvent, build_claude_settings_json, start_server, substitute_placeholder,
    temp_settings_dir, write_settings_file,
};

#[test]
fn agent_status_priority_orders_attention_first() {
    // Higher priority = more urgent. Notify is the most attention-worthy.
    assert!(AgentStatus::Notify.priority() > AgentStatus::Done.priority());
    assert!(AgentStatus::Done.priority() > AgentStatus::Working.priority());
    assert!(AgentStatus::Working.priority() > AgentStatus::Idle.priority());
}

#[test]
fn agent_status_from_str_round_trips_known() {
    for &(s, expected) in &[
        ("notify", AgentStatus::Notify),
        ("done", AgentStatus::Done),
        ("working", AgentStatus::Working),
        ("idle", AgentStatus::Idle),
    ] {
        assert_eq!(AgentStatus::from_wire(s), Some(expected));
    }
}

#[test]
fn agent_status_from_str_unknown_is_none() {
    assert_eq!(AgentStatus::from_wire(""), None);
    assert_eq!(AgentStatus::from_wire("bogus"), None);
    assert_eq!(AgentStatus::from_wire("NOTIFY"), None); // case-sensitive
}

#[test]
fn claude_settings_json_wires_all_status_hooks() {
    let json = build_claude_settings_json("http://127.0.0.1:51234", "tab-abc");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Every status-bearing event has a curl POST to the right URL.
    let expectations = [
        ("PermissionRequest", "notify"),
        ("Stop", "done"),
        ("UserPromptSubmit", "working"),
        ("PreToolUse", "working"),
        ("PostToolUse", "working"),
        ("SessionStart", "idle"),
    ];
    for (event, status) in expectations {
        let cmd = parsed
            .pointer(&format!("/hooks/{event}/0/hooks/0/command"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("missing {event} hook"));
        let expected_url = format!("http://127.0.0.1:51234/event/tab-abc/{status}");
        assert!(
            cmd.contains(&expected_url),
            "{event}: expected URL {expected_url} in cmd: {cmd}",
        );
        assert!(cmd.contains("curl"), "{event}: cmd should curl: {cmd}");
    }
}

#[test]
fn claude_settings_json_does_not_subscribe_to_notification() {
    // Claude Code's Notification hook fires on a timer for idle_prompt,
    // which would spuriously flip a freshly-cleared session back to notify.
    // PermissionRequest covers the legitimate permission case; matches
    // agent-status's reasoning.
    let json = build_claude_settings_json("http://127.0.0.1:1", "x");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.pointer("/hooks/Notification").is_none());
}

#[test]
fn substitute_placeholder_replaces_bare_form() {
    let out = substitute_placeholder("claude --settings $CLAUDE_SETTINGS", "/tmp/x.json");
    assert_eq!(out, "claude --settings /tmp/x.json");
}

#[test]
fn substitute_placeholder_replaces_braced_form() {
    let out = substitute_placeholder("claude --settings ${CLAUDE_SETTINGS}", "/tmp/x.json");
    assert_eq!(out, "claude --settings /tmp/x.json");
}

#[test]
fn substitute_placeholder_leaves_command_alone_when_absent() {
    let out = substitute_placeholder("claude", "/tmp/x.json");
    assert_eq!(out, "claude");
}

#[test]
fn substitute_placeholder_handles_multiple_occurrences() {
    let out = substitute_placeholder("a $CLAUDE_SETTINGS b ${CLAUDE_SETTINGS} c", "/p");
    assert_eq!(out, "a /p b /p c");
}

#[test]
fn parse_event_path_extracts_tab_and_event() {
    assert_eq!(
        parse_event_path("/event/tab-abc/notify"),
        Some(("tab-abc", "notify")),
    );
}

#[test]
fn parse_event_path_rejects_wrong_prefix() {
    assert_eq!(parse_event_path("/other/x/y"), None);
    assert_eq!(parse_event_path("event/x/y"), None);
}

#[test]
fn parse_event_path_rejects_missing_event_segment() {
    assert_eq!(parse_event_path("/event/tab-abc"), None);
    assert_eq!(parse_event_path("/event/tab-abc/"), None);
}

#[test]
fn parse_event_path_rejects_extra_segments() {
    // Three segments after /event/ would let a hook write to a nested path
    // we don't recognise — reject explicitly.
    assert_eq!(parse_event_path("/event/tab-abc/notify/extra"), None);
}

#[test]
fn parse_event_path_strips_query_string() {
    assert_eq!(
        parse_event_path("/event/tab-abc/notify?foo=bar"),
        Some(("tab-abc", "notify")),
    );
}

#[test]
fn server_receives_post_and_dispatches_hook_event() {
    use futures::StreamExt;
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::Duration;

    let (port, mut rx) = start_server().expect("start server");

    // Open the TCP socket directly so the test doesn't depend on curl.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .write_all(
            b"POST /event/tab-xyz/notify HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        )
        .unwrap();
    drop(stream);

    // Pump the futures channel until we get our event or time out.
    let runtime = futures::executor::block_on(async {
        futures::future::select(
            Box::pin(rx.next()),
            Box::pin(async {
                futures_timer::Delay::new(Duration::from_secs(2)).await;
                Option::<HookEvent>::None
            }),
        )
        .await
    });
    let event = match runtime {
        futures::future::Either::Left((Some(e), _)) => e,
        _ => panic!("did not receive hook event"),
    };
    assert_eq!(event.tab_id, "tab-xyz");
    assert_eq!(event.event, "notify");
}

#[test]
fn aggregate_status_returns_none_when_empty() {
    let entries: Vec<(PathBuf, AgentStatus)> = vec![];
    let target = PathBuf::from("/ws/a");
    assert_eq!(
        aggregate_status(entries.iter().map(|(p, s)| (p.as_path(), *s)), &target),
        None,
    );
}

#[test]
fn aggregate_status_picks_highest_priority_for_workspace() {
    let entries = vec![
        (PathBuf::from("/ws/a"), AgentStatus::Idle),
        (PathBuf::from("/ws/a"), AgentStatus::Notify),
        (PathBuf::from("/ws/a"), AgentStatus::Working),
        (PathBuf::from("/ws/b"), AgentStatus::Done),
    ];
    assert_eq!(
        aggregate_status(
            entries.iter().map(|(p, s)| (p.as_path(), *s)),
            &PathBuf::from("/ws/a"),
        ),
        Some(AgentStatus::Notify),
    );
    assert_eq!(
        aggregate_status(
            entries.iter().map(|(p, s)| (p.as_path(), *s)),
            &PathBuf::from("/ws/b"),
        ),
        Some(AgentStatus::Done),
    );
    assert_eq!(
        aggregate_status(
            entries.iter().map(|(p, s)| (p.as_path(), *s)),
            &PathBuf::from("/ws/c"),
        ),
        None,
    );
}

#[test]
fn temp_settings_dir_is_pid_scoped() {
    let dir = temp_settings_dir();
    let pid = std::process::id();
    assert!(
        dir.file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.contains(&pid.to_string()))
            .unwrap_or(false),
        "expected temp dir to include pid {pid}, got {dir:?}",
    );
}

#[test]
fn write_settings_file_creates_parent_and_writes_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("tab-xyz.json");
    let json = r#"{"hooks":{}}"#;
    write_settings_file(&path, json).unwrap();
    let read_back = std::fs::read_to_string(&path).unwrap();
    assert_eq!(read_back, json);
}

#[test]
fn write_settings_file_overwrites_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("tab.json");
    write_settings_file(&path, "first").unwrap();
    write_settings_file(&path, "second").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
}
