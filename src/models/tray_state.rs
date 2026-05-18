use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrayState {
    pub last_check: Option<DateTime<Utc>>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub aur: Vec<String>,
    #[serde(default)]
    pub flatpak: Vec<String>,
}

impl TrayState {
    pub fn total(&self) -> usize {
        return self.packages.len() + self.aur.len() + self.flatpak.len();
    }
}

pub fn state_dir() -> Option<PathBuf> {
    if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
        if !state_home.is_empty() {
            return Some(PathBuf::from(state_home).join("arch-update-manager"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".local/state/arch-update-manager"));
    }
    return None;
}

pub fn state_file() -> Option<PathBuf> {
    return state_dir().map(|d| d.join("updates.json"));
}
