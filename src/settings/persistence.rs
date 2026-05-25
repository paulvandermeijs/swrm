use super::AppSettings;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("no config dir on this platform")?;
    Ok(dir.join("swrm").join("settings.json"))
}

pub fn load_from(path: &Path) -> Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    match serde_json::from_str::<AppSettings>(&raw) {
        Ok(settings) => Ok(settings),
        Err(err) => {
            tracing::warn!(
                ?err,
                path = %path.display(),
                "malformed settings.json, falling back to default"
            );
            Ok(AppSettings::default())
        }
    }
}

pub fn save_to(path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(path, raw)?;
    Ok(())
}

pub fn load() -> Result<AppSettings> {
    load_from(&config_path()?)
}

pub fn save(settings: &AppSettings) -> Result<()> {
    save_to(&config_path()?, settings)
}
