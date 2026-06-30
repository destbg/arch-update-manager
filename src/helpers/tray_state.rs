use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;

use crate::helpers::elevated::chown_to_user;
use crate::models::package_source::PackageSource;
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
        appimage: Vec::new(),
    };
    for pkg in packages {
        let entry = format!(
            "{} {} -> {}",
            pkg.name, pkg.current_version, pkg.new_version
        );
        match pkg.source {
            PackageSource::Aur => state.aur.push(entry),
            PackageSource::Flatpak => state.flatpak.push(entry),
            PackageSource::AppImage => state.appimage.push(entry),
            PackageSource::Official => state.packages.push(entry),
        }
    }
    return state;
}

pub fn write_tray_state(state: &TrayState) -> Result<()> {
    let dir = state_dir().ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))?;
    fs::create_dir_all(&dir)?;
    if let Some(parent) = dir.parent() {
        chown_to_user(parent);
    }
    chown_to_user(&dir);

    let path =
        state_file().ok_or_else(|| anyhow::anyhow!("Could not determine state file path"))?;
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content)?;
    chown_to_user(&path);
    return Ok(());
}
