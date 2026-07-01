use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::helpers::elevated::chown_to_user;
use crate::models::appimage_entry::AppImageEntry;
use crate::models::appimage_update_source::AppImageUpdateSource;

pub fn load_appimage_entries() -> Vec<AppImageEntry> {
    let Some(path) = config_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    return serde_json::from_str(&content).unwrap_or_default();
}

pub fn save_appimage_entries(entries: &[AppImageEntry]) -> Result<()> {
    let path = config_path().context("Could not determine config directory")?;
    let content =
        serde_json::to_string_pretty(entries).context("Failed to serialize AppImage sources")?;
    fs::write(&path, content).context("Failed to write AppImage sources file")?;
    chown_to_user(&path);
    return Ok(());
}

pub fn source_for_path(path: &str) -> Option<AppImageUpdateSource> {
    return load_appimage_entries()
        .into_iter()
        .find(|entry| entry.path == path)
        .map(|entry| entry.source);
}

pub fn set_source_for_path(path: &str, name: &str, source: AppImageUpdateSource) -> Result<()> {
    let mut entries = load_appimage_entries();
    if let Some(existing) = entries.iter_mut().find(|entry| entry.path == path) {
        existing.name = name.to_string();
        existing.source = source;
    } else {
        entries.push(AppImageEntry {
            path: path.to_string(),
            name: name.to_string(),
            source,
        });
    }
    return save_appimage_entries(&entries);
}

pub fn remove_appimage_entry(path: &str) -> Result<()> {
    let mut entries = load_appimage_entries();
    let before = entries.len();
    entries.retain(|entry| entry.path != path);
    if entries.len() == before {
        return Ok(());
    }
    return save_appimage_entries(&entries);
}

fn config_path() -> Option<PathBuf> {
    let config_dir = if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config_home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        return None;
    };

    let app_config_dir = config_dir.join("arch-update-manager");
    if !app_config_dir.exists() {
        if fs::create_dir_all(&app_config_dir).is_err() {
            return None;
        }
        chown_to_user(&app_config_dir);
    }

    return Some(app_config_dir.join("appimages.json"));
}
