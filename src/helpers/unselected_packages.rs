use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::helpers::elevated::chown_to_user;

pub fn load_unselected_packages() -> Vec<String> {
    let path = match unselected_packages_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    if !path.exists() {
        return Vec::new();
    }

    return match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
}

pub fn save_unselected_packages(packages: Vec<String>) {
    let path = match unselected_packages_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to get unselected packages path: {}", e);
            return;
        }
    };

    let content = match serde_json::to_string(&packages) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to serialize unselected packages: {}", e);
            return;
        }
    };

    if let Err(e) = fs::write(&path, content) {
        eprintln!("Failed to save unselected packages: {}", e);
        return;
    }
    chown_to_user(&path);
}

fn unselected_packages_path() -> Result<PathBuf> {
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
        chown_to_user(&app_config_dir);
    }

    return Ok(app_config_dir.join("unselected.json"));
}
