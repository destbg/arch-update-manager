use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::helpers::elevated::{get_original_user, spawn_as_user_or_root};

const AUTOSTART_FILENAME: &str = "arch-update-manager-tray.desktop";
const TIMER_UNIT: &str = "arch-update-manager-check.timer";
const TRAY_BINARY: &str = "arch-update-manager-tray";
const TRAY_DESKTOP_CONTENT: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Arch Update Manager Tray\n\
Comment=System tray applet for Arch Update Manager\n\
Exec=arch-update-manager-tray\n\
Icon=arch-update-manager\n\
Terminal=false\n\
Categories=System;PackageManager;\n\
X-GNOME-Autostart-enabled=true\n";

pub fn apply_tray_state(enabled: bool) {
    if enabled {
        enable_tray();
    } else {
        disable_tray();
    }
}

fn enable_tray() {
    if let Err(e) = write_autostart_file() {
        eprintln!("Failed to write autostart entry: {}", e);
    }

    run_user_systemctl(&["enable", "--now", TIMER_UNIT]);

    if !is_tray_running() {
        spawn_as_user_or_root(TRAY_BINARY, &[]);
    }
}

fn disable_tray() {
    if let Err(e) = remove_autostart_file() {
        eprintln!("Failed to remove autostart entry: {}", e);
    }

    run_user_systemctl(&["disable", "--now", TIMER_UNIT]);

    kill_running_tray();
}

fn autostart_dir() -> Option<PathBuf> {
    if let Some(home) = user_home() {
        return Some(PathBuf::from(home).join(".config/autostart"));
    }
    return None;
}

fn user_home() -> Option<String> {
    if let Some(user) = get_original_user() {
        return Some(format!("/home/{}", user));
    }
    return std::env::var("HOME").ok();
}

fn write_autostart_file() -> std::io::Result<()> {
    let Some(dir) = autostart_dir() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine autostart directory",
        ));
    };
    fs::create_dir_all(&dir)?;
    let path = dir.join(AUTOSTART_FILENAME);
    fs::write(&path, TRAY_DESKTOP_CONTENT)?;
    chown_to_user(&path);
    if let Some(parent) = path.parent() {
        chown_to_user(&parent.to_path_buf());
    }
    return Ok(());
}

fn remove_autostart_file() -> std::io::Result<()> {
    let Some(dir) = autostart_dir() else {
        return Ok(());
    };
    let path = dir.join(AUTOSTART_FILENAME);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    return Ok(());
}

fn chown_to_user(path: &PathBuf) {
    let Some(user) = get_original_user() else {
        return;
    };
    let target = format!("{}:{}", user, user);
    let _ = Command::new("chown").arg(&target).arg(path).status();
}

fn run_user_systemctl(args: &[&str]) {
    let Some(user) = get_original_user() else {
        let _ = Command::new("systemctl").arg("--user").args(args).status();
        return;
    };

    let uid = match user_uid(&user) {
        Some(uid) => uid,
        None => return,
    };

    let xdg_runtime = format!("XDG_RUNTIME_DIR=/run/user/{}", uid);
    let dbus_addr = format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{}/bus", uid);

    let mut sudo_args: Vec<String> = vec![
        "-u".to_string(),
        user.clone(),
        xdg_runtime,
        dbus_addr,
        "systemctl".to_string(),
        "--user".to_string(),
    ];
    for arg in args {
        sudo_args.push((*arg).to_string());
    }

    let _ = Command::new("sudo").args(&sudo_args).status();
}

fn user_uid(user: &str) -> Option<String> {
    let output = Command::new("id").args(["-u", user]).output().ok()?;
    let uid = String::from_utf8(output.stdout).ok()?;
    let uid = uid.trim().to_string();
    if uid.is_empty() {
        return None;
    }
    return Some(uid);
}

fn is_tray_running() -> bool {
    let Some(user) = get_original_user() else {
        return false;
    };
    let output = Command::new("pgrep")
        .args(["-u", &user, "-x", TRAY_BINARY])
        .output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

fn kill_running_tray() {
    let Some(user) = get_original_user() else {
        let _ = Command::new("pkill").args(["-x", TRAY_BINARY]).status();
        return;
    };
    let _ = Command::new("pkill")
        .args(["-u", &user, "-x", TRAY_BINARY])
        .status();
}
