use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn validate_repo(path: &Path) -> Result<gix::Repository> {
    gix::open(path).with_context(|| format!("{} is not a git repository", path.display()))
}

pub fn current_branch(repo: &gix::Repository) -> Option<String> {
    repo.head_name().ok().flatten().map(|name| name.shorten().to_string())
}

pub fn list_worktrees(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    out.push(repo.workdir().context("bare repo not supported")?.to_path_buf());
    for proxy in repo.worktrees()? {
        if let Ok(repo) = proxy.into_repo() {
            if let Some(dir) = repo.workdir() {
                out.push(dir.to_path_buf());
            }
        }
    }
    Ok(out)
}

pub fn create_worktree(repo: &gix::Repository, branch: &str, target_dir: &Path) -> Result<PathBuf> {
    let workdir = repo.workdir().context("bare repo not supported")?;
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("worktree")
        .arg("add")
        .arg(target_dir)
        .arg(branch)
        .status()
        .context("spawn `git worktree add`")?;
    anyhow::ensure!(status.success(), "git worktree add failed");
    Ok(target_dir.to_path_buf())
}
