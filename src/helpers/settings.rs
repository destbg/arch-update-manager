use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::helpers::aur::{is_command_available, pamac_supports_aur, shelly_supports_aur};
use crate::helpers::elevated::chown_to_user;
use crate::models::app_settings::AppSettings;
use crate::models::check_schedule::CheckSchedule;
use crate::models::snapshot_retention_period::SnapshotRetentionPeriod;

static SETTINGS_CACHE: OnceLock<Mutex<AppSettings>> = OnceLock::new();

fn default_settings() -> AppSettings {
    return AppSettings {
        enable_aur_support: false,
        preferred_aur_helper: None,
        create_timeshift_snapshot: is_command_available("timeshift"),
        snapshot_retention_count: 1,
        snapshot_retention_period: SnapshotRetentionPeriod::Forever,
        separate_repository_groups: false,
        separate_repositories: Vec::new(),
        remember_unselected_packages: true,
        enable_favorites: true,
        show_favorites_column: false,
        favorite_packages: Vec::new(),
        favorites_exclusion_mode: false,
        enable_flatpak_support: is_command_available("flatpak"),
        enable_appimage_support: true,
        enable_devel_aur: false,
        keep_old_packages: 3,
        keep_uninstalled_packages: 0,
        auto_clean_cache: false,
        run_post_update_checks: true,
        create_snapper_snapshot: false,
        enable_system_tray: false,
        check_schedule: CheckSchedule::Daily,
        tray_always_visible: false,
        tray_only_favorites: false,
        tray_menu_only_favorites: false,
        skip_check_on_metered: false,
        skip_check_on_battery: false,
        show_update_notifications: false,
        show_package_descriptions: true,
        show_updated_date: true,
        min_update_age_days: 0,
        min_update_age_aur_only: false,
        log_retention_days: 7,
        check_arch_news: true,
        enable_mirror_refresh: true,
    };
}

pub fn load_settings() -> AppSettings {
    let cache = SETTINGS_CACHE.get_or_init(|| {
        let settings = match load_from_file() {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!("Failed to load settings: {}, using defaults", e);
                default_settings()
            }
        };
        Mutex::new(settings)
    });

    return cache.lock().unwrap().clone();
}

pub fn reload_settings() -> AppSettings {
    let fresh = load_from_file().unwrap_or_else(|e| {
        eprintln!("Failed to reload settings: {}, using defaults", e);
        default_settings()
    });
    let cache = SETTINGS_CACHE.get_or_init(|| Mutex::new(fresh.clone()));
    if let Ok(mut cached) = cache.lock() {
        *cached = fresh.clone();
    }
    return fresh;
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let path = settings_path()?;

    let content = serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

    fs::write(&path, content).context("Failed to write settings file")?;
    chown_to_user(&path);

    if let Some(cache) = SETTINGS_CACHE.get() {
        if let Ok(mut cached_settings) = cache.lock() {
            *cached_settings = settings.clone();
        }
    }

    return Ok(());
}

pub fn get_available_aur_helpers() -> Vec<String> {
    let helpers = ["yay", "paru", "trizen", "pikaur", "shelly", "pamac"];
    let mut available = Vec::new();

    for helper in &helpers {
        if !is_command_available(helper) {
            continue;
        }
        if *helper == "pamac" && !pamac_supports_aur() {
            continue;
        }
        if *helper == "shelly" && !shelly_supports_aur() {
            continue;
        }
        available.push(helper.to_string());
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

    let helpers = ["yay", "paru", "trizen", "pikaur", "shelly", "pamac"];
    for helper in &helpers {
        if !is_command_available(helper) {
            continue;
        }
        if *helper == "pamac" && !pamac_supports_aur() {
            continue;
        }
        if *helper == "shelly" && !shelly_supports_aur() {
            continue;
        }
        return Some(helper.to_string());
    }

    return None;
}

fn load_from_file() -> Result<AppSettings> {
    let path = settings_path()?;

    if !path.exists() {
        return Ok(default_settings());
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

    chown_to_user(&app_config_dir);

    return Ok(app_config_dir.join("settings.json"));
}
