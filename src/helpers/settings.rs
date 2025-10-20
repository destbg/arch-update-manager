use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::helpers::aur::is_command_available;
use crate::models::app_settings::AppSettings;

static SETTINGS_CACHE: OnceLock<Mutex<AppSettings>> = OnceLock::new();

pub fn load_settings() -> AppSettings {
    let cache = SETTINGS_CACHE.get_or_init(|| {
        let settings = match load_from_file() {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!("Failed to load settings: {}, using defaults", e);
                AppSettings {
                    enable_aur_support: false,
                    preferred_aur_helper: None,
                    create_timeshift_snapshot: true,
                }
            }
        };
        Mutex::new(settings)
    });

    return cache.lock().unwrap().clone();
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let path = settings_path()?;

    let content = serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

    fs::write(&path, content).context("Failed to write settings file")?;

    if let Some(cache) = SETTINGS_CACHE.get() {
        if let Ok(mut cached_settings) = cache.lock() {
            *cached_settings = settings.clone();
        }
    }

    return Ok(());
}

pub fn get_available_aur_helpers() -> Vec<String> {
    let helpers = ["yay", "paru", "trizen", "pikaur", "pamac"];
    let mut available = Vec::new();

    for helper in &helpers {
        if is_command_available(helper) {
            available.push(helper.to_string());
        }
    }

    return available;
}

pub fn get_effective_aur_helper(settings: &AppSettings) -> Option<String> {
    if !settings.enable_aur_support {
        return None;
    }

    if let Some(ref preferred) = settings.preferred_aur_helper {
        if is_command_available(preferred) {
            return Some(preferred.clone());
        }
    }

    let helpers = ["yay", "paru", "trizen", "pikaur", "pamac"];
    for helper in &helpers {
        if is_command_available(helper) {
            return Some(helper.to_string());
        }
    }

    return None;
}

fn load_from_file() -> Result<AppSettings> {
    let path = settings_path()?;

    if !path.exists() {
        return Ok(AppSettings {
            enable_aur_support: false,
            preferred_aur_helper: None,
            create_timeshift_snapshot: true,
        });
    }

    let content = fs::read_to_string(&path).context("Failed to read settings file")?;

    let settings: AppSettings =
        serde_json::from_str(&content).context("Failed to parse settings file")?;

    return Ok(settings);
}

fn settings_path() -> Result<PathBuf> {
    let config_dir = if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config_home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        return Err(anyhow::anyhow!("Could not determine config directory"));
    };

    let app_config_dir = config_dir.join("arch-update-manager");

    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
    }

    return Ok(app_config_dir.join("settings.json"));
}
