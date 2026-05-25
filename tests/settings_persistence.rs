use swrm::settings::{Agent, AppSettings, persistence};
use tempfile::tempdir;

#[test]
fn round_trips_agents() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let settings = AppSettings {
        agents: vec![
            Agent {
                id: "agent-1".into(),
                name: "claude".into(),
                command: "claude".into(),
            },
            Agent {
                id: "agent-2".into(),
                name: "codex".into(),
                command: "codex --print".into(),
            },
        ],
    };
    persistence::save_to(&path, &settings).unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(loaded, settings);
}

#[test]
fn missing_file_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.json");
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(loaded, AppSettings::default());
}

#[test]
fn malformed_json_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, b"not json at all").unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(
        loaded,
        AppSettings::default(),
        "malformed JSON should fall back to default"
    );
}

#[test]
fn missing_agents_key_loads_as_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.json");
    std::fs::write(&path, b"{}").unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(loaded, AppSettings::default());
}
