use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Unified diff of `file` in the worktree at `repo_path` vs HEAD.
/// Shells out to `git diff` to sidestep gix's evolving diff API.
pub fn diff_file(repo_path: &Path, file: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("diff")
        .arg("--no-color")
        .arg("--")
        .arg(file)
        .output()
        .context("spawn git diff")?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
