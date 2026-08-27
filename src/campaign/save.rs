//! JSON save-file persistence for [`crate::campaign::model::CampaignState`].

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::campaign::model::CampaignState;

pub struct SaveFile {
    path: PathBuf,
}

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(error) => write!(f, "save file error: {error}"),
            SaveError::Json(error) => write!(f, "corrupted save file: {error}"),
        }
    }
}

impl From<io::Error> for SaveError {
    fn from(error: io::Error) -> Self {
        SaveError::Io(error)
    }
}

impl From<serde_json::Error> for SaveError {
    fn from(error: serde_json::Error) -> Self {
        SaveError::Json(error)
    }
}

impl SaveFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn platform_default() -> Self {
        Self::new(platform_default_path())
    }

    pub fn load(&self) -> Result<Option<CampaignState>, SaveError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(SaveError::Io(error)),
        }
    }

    pub fn store(&self, state: &CampaignState) -> Result<(), SaveError> {
        let bytes = serde_json::to_vec_pretty(state)?;
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        // Atomic replace: write a sibling temp file, sync it, then rename over
        // the live save. A failed write leaves the previous save intact.
        let temp = sibling_temp_path(&self.path);
        let stored = (|| -> Result<(), SaveError> {
            let mut file = fs::File::create(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if stored.is_err() {
            let _ = fs::remove_file(&temp);
        }
        stored
    }
}

fn sibling_temp_path(path: &std::path::Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(target_os = "macos")]
fn platform_default_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/Scorpius/campaign.json"))
        .unwrap_or_else(|| PathBuf::from("campaign.json"))
}

#[cfg(target_os = "windows")]
fn platform_default_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(|appdata| {
            PathBuf::from(appdata)
                .join("Scorpius")
                .join("campaign.json")
        })
        .unwrap_or_else(|| PathBuf::from("campaign.json"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_default_path() -> PathBuf {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(xdg) if !xdg.is_empty() => PathBuf::from(xdg).join("scorpius").join("campaign.json"),
        _ => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share/scorpius/campaign.json"))
            .unwrap_or_else(|| PathBuf::from("campaign.json")),
    }
}
