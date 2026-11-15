use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Result;
use std::path::PathBuf;
use std::process::{Command, Output};

use crate::helpers::elevated::{chown_to_user, get_original_user, get_user_home};
use crate::models::check_schedule::CheckSchedule;

const LEGACY_AUTOSTART_FILENAME: &str = "arch-update-manager-tray.desktop";
const TIMER_UNIT: &str = "arch-update-manager-check.timer";
const CHECK_SERVICE: &str = "arch-update-manager-check.service";
const TRAY_SERVICE: &str = "arch-update-manager-tray.service";

pub fn trigger_check_service() {
    std::thread::spawn(|| {
        run_user_systemctl(&["start", CHECK_SERVICE]);
    });
}

pub fn kick_tray() {
    let _ = Command::new("pkill")
        .args(["-USR1", "-f", "arch-update-manager-tray"])
        .status();
}

pub fn apply_tray_state(enabled: bool) {
    remove_legacy_autostart_file();

    if enabled {
        run_user_systemctl(&["enable", "--now", "--no-block", TIMER_UNIT]);
        run_user_systemctl(&["enable", "--now", "--no-block", TRAY_SERVICE]);
    } else {
        run_user_systemctl(&["disable", "--now", TIMER_UNIT]);
        run_user_systemctl(&["disable", "--now", TRAY_SERVICE]);
    }
}

pub fn apply_check_schedule(schedule: CheckSchedule) {
    if let Err(e) = write_check_timer_override(schedule) {
        eprintln!("Failed to write check timer override: {}", e);
        return;
    }
    run_user_systemctl(&["daemon-reload"]);
    run_user_systemctl(&["try-restart", TIMER_UNIT]);
}

pub fn has_systemd_user_session() -> bool {
    let Some(uid) = original_user_uid() else {
        return false;
    };
    return PathBuf::from(format!("/run/user/{}/systemd", uid)).exists();
}

fn write_check_timer_override(schedule: CheckSchedule) -> std::io::Result<()> {
    let Some(config) = user_config_dir() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine the configuration folder",
        ));
    };

    let dir = config
        .join("systemd/user")
        .join(format!("{}.d", TIMER_UNIT));
    fs::create_dir_all(&dir)?;

    let path = dir.join("override.conf");
    let contents = format!(
        "[Timer]\nOnStartupSec=\nOnUnitActiveSec=\nOnCalendar=\nOnCalendar={}\nPersistent=true\n",
        schedule.to_oncalendar()
    );
    fs::write(&path, contents)?;

    chown_to_user(&dir);
    chown_to_user(&path);
    return Ok(());
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
    return Some(user_config_dir()?.join("autostart"));
}

fn user_config_dir() -> Option<PathBuf> {
    if let Some(config) = env::var_os("XDG_CONFIG_HOME") {
        let config = PathBuf::from(config);
        if config.is_absolute() {
            return Some(config);
        }
    }
    return Some(PathBuf::from(user_home()?).join(".config"));
}

fn user_home() -> Option<String> {
    if let Some(user) = get_original_user() {
        return get_user_home(&user);
    }
    return env::var("HOME").ok();
}

fn run_user_systemctl(args: &[&str]) {
    let Some(config) = user_config_dir() else {
        return;
    };
    let Some(user) = get_original_user() else {
        let output = Command::new("systemctl")
            .env("XDG_CONFIG_HOME", &config)
            .arg("--user")
            .args(args)
            .output();
        log_systemctl_result(args, &output);
        return;
    };

    let Some(uid) = user_uid(&user) else {
        return;
    };

    let xdg_runtime = format!("XDG_RUNTIME_DIR=/run/user/{}", uid);
    let dbus_addr = format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{}/bus", uid);

    let mut xdg_config = OsString::from("XDG_CONFIG_HOME=");
    xdg_config.push(config.as_os_str());
    let output = Command::new("sudo")
        .args(["-u", &user, &xdg_runtime, &dbus_addr])
        .arg(xdg_config)
        .args(["systemctl", "--user"])
        .args(args)
        .output();
    log_systemctl_result(args, &output);
}

fn log_systemctl_result(args: &[&str], output: &Result<Output>) {
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
