use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::helpers::elevated::get_original_user;

const LEGACY_AUTOSTART_FILENAME: &str = "arch-update-manager-tray.desktop";
const TIMER_UNIT: &str = "arch-update-manager-check.timer";
const CHECK_SERVICE: &str = "arch-update-manager-check.service";
const TRAY_SERVICE: &str = "arch-update-manager-tray.service";

pub fn trigger_check_service() {
    run_user_systemctl(&["start", CHECK_SERVICE]);
}

pub fn kick_tray() {
    let _ = Command::new("pkill")
        .args(["-USR1", "-f", "arch-update-manager-tray"])
        .status();
}

pub fn apply_tray_state(enabled: bool) {
    remove_legacy_autostart_file();

    if enabled {
        run_user_systemctl(&["enable", "--now", TIMER_UNIT]);
        run_user_systemctl(&["enable", "--now", TRAY_SERVICE]);
    } else {
        run_user_systemctl(&["disable", "--now", TIMER_UNIT]);
        run_user_systemctl(&["disable", "--now", TRAY_SERVICE]);
    }
}

pub fn has_systemd_user_session() -> bool {
    let Some(uid) = original_user_uid() else {
        return false;
    };
    return PathBuf::from(format!("/run/user/{}/systemd", uid)).exists();
}

fn remove_legacy_autostart_file() {
    let Some(dir) = autostart_dir() else {
        return;
    };
    let path = dir.join(LEGACY_AUTOSTART_FILENAME);
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
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

fn run_user_systemctl(args: &[&str]) {
    let Some(user) = get_original_user() else {
        let output = Command::new("systemctl").arg("--user").args(args).output();
        log_systemctl_result(args, output);
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

    let output = Command::new("sudo").args(&sudo_args).output();
    log_systemctl_result(args, output);
}

fn log_systemctl_result(args: &[&str], output: std::io::Result<std::process::Output>) {
    match output {
        Ok(o) if !o.status.success() => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let stdout = String::from_utf8_lossy(&o.stdout);
            eprintln!(
                "systemctl --user {} failed ({}): {}{}",
                args.join(" "),
                o.status,
                stderr.trim(),
                if stdout.trim().is_empty() {
                    String::new()
                } else {
                    format!(" / {}", stdout.trim())
                }
            );
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("Failed to run systemctl --user {}: {}", args.join(" "), e);
        }
    }
}

fn original_user_uid() -> Option<String> {
    let user = get_original_user()?;
    return user_uid(&user);
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
