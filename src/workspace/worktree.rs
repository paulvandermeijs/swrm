use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub fn validate_repo(path: &Path) -> Result<gix::Repository> {
    gix::open(path).with_context(|| format!("{} is not a git repository", path.display()))
}

pub fn current_branch(repo: &gix::Repository) -> Option<String> {
    repo.head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string())
}

/// Returns the workdir of the main repository for the given (possibly worktree)
/// repository — i.e. the parent of `common_dir`. Workspaces that share a
/// project_dir are worktrees of the same repo.
///
/// `common_dir` is canonicalized first because for a linked worktree gix
/// returns it unresolved as `<main>/.git/worktrees/<name>/../..`; taking the
/// parent of that raw path yields `<main>/.git/worktrees/<name>/..` which
/// resolves to `<main>/.git/worktrees` — not the project root we want.
pub fn project_dir(repo: &gix::Repository) -> PathBuf {
    let common = repo.common_dir();
    let canonical = std::fs::canonicalize(common).unwrap_or_else(|_| common.to_path_buf());
    canonical.parent().unwrap_or(&canonical).to_path_buf()
}

pub fn list_worktrees(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    out.push(
        repo.workdir()
            .context("bare repo not supported")?
            .to_path_buf(),
    );
    for proxy in repo.worktrees()? {
        if let Ok(repo) = proxy.into_repo() {
            if let Some(dir) = repo.workdir() {
                out.push(dir.to_path_buf());
            }
        }
    }
    Ok(out)
}

#[must_use]
pub fn random_name() -> String {
    petname::petname(2, "-").expect("petname wordlist is non-empty")
}

/// Create a new worktree with branch `name` under `<project>/.worktrees/<name>`.
/// Idempotent: if the target already exists, returns its path.
pub fn ensure_worktree(repo: &gix::Repository, name: &str) -> Result<PathBuf> {
    let project = project_dir(repo);
    let target = project.join(".worktrees").join(name);
    if target.is_dir() {
        return Ok(target);
    }
    let parent = target.parent().expect(".worktrees has a parent");
    std::fs::create_dir_all(parent)
        .with_context(|| format!("could not create {}", parent.display()))?;
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(&project)
        .args(["worktree", "add", "-b", name])
        .arg(&target)
        .output()
        .context("failed to spawn `git worktree add`")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git worktree add failed (exit status {}): {}",
            output.status,
            stderr.trim()
        );
    }
    Ok(target)
}
