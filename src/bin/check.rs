use std::fs;
use std::process::Command;

use chrono::Utc;

use arch_update_manager::helpers::aur::get_aur_updates;
use arch_update_manager::helpers::flatpak::get_flatpak_updates;
use arch_update_manager::helpers::settings::load_settings;
use arch_update_manager::models::tray_state::{TrayState, state_dir, state_file};

fn main() {
    let settings = load_settings();

    let packages = get_repo_updates().unwrap_or_else(|e| {
        eprintln!("Failed to get repo updates: {}", e);
        Vec::new()
    });

    let aur = if settings.enable_aur_support {
        match get_aur_updates() {
            Ok(updates) => updates
                .into_iter()
                .map(|u| format!("{} {} -> {}", u.name, u.current_version, u.new_version))
                .collect(),
            Err(e) => {
                eprintln!("Failed to get AUR updates: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let flatpak = if settings.enable_flatpak_support {
        match get_flatpak_updates() {
            Ok(updates) => updates
                .into_iter()
                .map(|u| format!("{} {} -> {}", u.name, u.current_version, u.new_version))
                .collect(),
            Err(e) => {
                eprintln!("Failed to get Flatpak updates: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let state = TrayState {
        last_check: Some(Utc::now()),
        packages,
        aur,
        flatpak,
    };

    if let Err(e) = write_state(&state) {
        eprintln!("Failed to write state file: {}", e);
        std::process::exit(1);
    }
}

fn get_repo_updates() -> anyhow::Result<Vec<String>> {
    let output = Command::new("checkupdates").output()?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        if code == 2 {
            return Ok(Vec::new());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "checkupdates failed ({}): {}",
            code,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut updates = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            updates.push(trimmed.to_string());
        }
    }
    return Ok(updates);
}

fn write_state(state: &TrayState) -> anyhow::Result<()> {
    let dir = state_dir().ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))?;
    fs::create_dir_all(&dir)?;

    let path =
        state_file().ok_or_else(|| anyhow::anyhow!("Could not determine state file path"))?;
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content)?;
    return Ok(());
}
