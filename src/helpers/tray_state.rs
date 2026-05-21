use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;

use crate::constants::{AUR_NAME, FLATPAK_NAME};
use crate::models::package_update::PackageUpdate;
use crate::models::tray_state::TrayState;

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

pub fn build_tray_state(packages: &[PackageUpdate]) -> TrayState {
    let mut state = TrayState {
        last_check: Some(Utc::now()),
        packages: Vec::new(),
        aur: Vec::new(),
        flatpak: Vec::new(),
    };
    for pkg in packages {
        let entry = format!(
            "{} {} -> {}",
            pkg.name, pkg.current_version, pkg.new_version
        );
        if pkg.repository == AUR_NAME {
            state.aur.push(entry);
        } else if pkg.repository == FLATPAK_NAME {
            state.flatpak.push(entry);
        } else {
            state.packages.push(entry);
        }
    }
    return state;
}

pub fn write_tray_state(state: &TrayState) -> Result<()> {
    let dir = state_dir().ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))?;
    fs::create_dir_all(&dir)?;

    let path =
        state_file().ok_or_else(|| anyhow::anyhow!("Could not determine state file path"))?;
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content)?;
    return Ok(());
}
