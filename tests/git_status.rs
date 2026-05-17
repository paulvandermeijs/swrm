use std::fs;
use std::process::Command;
use swrm::git;
use tempfile::TempDir;

fn run(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git").args(args).current_dir(dir).status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn detects_modified_added_untracked() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    run(p, &["init", "-q", "-b", "main"]);
    run(p, &["config", "user.email", "t@t.t"]);
    run(p, &["config", "user.name", "t"]);
    fs::write(p.join("tracked.txt"), "a\n").unwrap();
    run(p, &["add", "tracked.txt"]);
    run(p, &["commit", "-q", "-m", "init"]);

    fs::write(p.join("tracked.txt"), "b\n").unwrap();
    fs::write(p.join("new.txt"), "x\n").unwrap();
    run(p, &["add", "new.txt"]);
    fs::write(p.join("untracked.txt"), "y\n").unwrap();

    let entries = git::collect_status(p).unwrap();
    let by_name: std::collections::HashMap<_, _> = entries
        .iter()
        .map(|e| (e.path.to_string_lossy().to_string(), &e.status))
        .collect();
    assert_eq!(by_name.get("tracked.txt"), Some(&&git::Status::Modified));
    assert_eq!(by_name.get("new.txt"), Some(&&git::Status::Added));
    assert_eq!(by_name.get("untracked.txt"), Some(&&git::Status::Untracked));
}
