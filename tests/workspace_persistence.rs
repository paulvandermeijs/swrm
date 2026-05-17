use std::path::PathBuf;
use swrm::workspace::{Workspace, persistence};
use tempfile::TempDir;

#[test]
fn round_trips_workspaces() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("workspaces.json");
    let original = vec![
        Workspace {
            label: "main".into(),
            path: PathBuf::from("/repos/foo"),
            branch: Some("main".into()),
            project: Some(PathBuf::from("/repos/foo")),
        },
        Workspace {
            label: "feature-x".into(),
            path: PathBuf::from("/repos/foo-worktrees/x"),
            branch: Some("feature-x".into()),
            project: Some(PathBuf::from("/repos/foo")),
        },
    ];
    persistence::save_to(&path, &original).unwrap();
    let loaded = persistence::load_from(&path).unwrap();
    assert_eq!(original, loaded);
}

#[test]
fn missing_file_returns_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("does-not-exist.json");
    assert!(persistence::load_from(&path).unwrap().is_empty());
}
