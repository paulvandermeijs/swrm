use super::Workspace;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("no config dir on this platform")?;
    Ok(dir.join("swrm").join("workspaces.json"))
}

pub fn load_from(path: &Path) -> Result<Vec<Workspace>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_to(path: &Path, workspaces: &[Workspace]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(workspaces)?;
    fs::write(path, raw)?;
    Ok(())
}

pub fn load() -> Result<Vec<Workspace>> {
    load_from(&config_path()?)
}

pub fn save(workspaces: &[Workspace]) -> Result<()> {
    save_to(&config_path()?, workspaces)
}
