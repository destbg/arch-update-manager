use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::helpers::elevated::chown_to_user;
use crate::models::appimage_entry::AppImageEntry;
use crate::models::appimage_update_source::AppImageUpdateSource;
use crate::models::shelly_appimage::ShellyAppImage;

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

pub fn shelly_db_path() -> Option<PathBuf> {
    let cache_dir = if let Ok(cache_home) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(cache_home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else {
        return None;
    };
    return Some(cache_dir.join("Shelly/appimage-local-meta-store/appimage-metadata-v2.db"));
}

pub fn shelly_has_appimage_data() -> bool {
    return shelly_db_path().map(|path| path.exists()).unwrap_or(false);
}

pub fn import_shelly_sources() -> Result<(usize, usize)> {
    let path = shelly_db_path().context("Could not find the shelly cache directory")?;
    let content =
        fs::read_to_string(&path).context("Could not read the shelly AppImage database")?;
    let entries: Vec<ShellyAppImage> =
        serde_json::from_str(&content).context("Could not parse the shelly AppImage database")?;

    let mut imported = 0;
    let mut skipped = 0;
    for entry in entries {
        let Some(entry_path) = entry.path.clone() else {
            skipped += 1;
            continue;
        };
        let source = shelly_to_source(&entry);
        if matches!(source, AppImageUpdateSource::None) {
            skipped += 1;
            continue;
        }
        if set_source_for_path(&entry_path, &entry.name, source).is_ok() {
            imported += 1;
        } else {
            skipped += 1;
        }
    }
    return Ok((imported, skipped));
}

fn shelly_to_source(entry: &ShellyAppImage) -> AppImageUpdateSource {
    return match entry.update_type {
        2 => match (&entry.repo_owner, &entry.repo_name) {
            (Some(owner), Some(repo)) if !owner.is_empty() && !repo.is_empty() => {
                AppImageUpdateSource::GitHub {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    prerelease: entry.allow_prerelease,
                }
            }
            _ => AppImageUpdateSource::None,
        },
        1 if entry.update_url.to_lowercase().ends_with(".zsync") => AppImageUpdateSource::Zsync {
            url: entry.update_url.clone(),
        },
        _ => AppImageUpdateSource::None,
    };
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
