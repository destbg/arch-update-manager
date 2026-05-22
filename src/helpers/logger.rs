use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::helpers::elevated::chown_to_user;
use crate::models::log_file::LogFile;
use crate::models::log_level::LogLevel;

static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init() {
    let dir = match logs_dir() {
        Some(d) => d,
        None => return,
    };

    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("Failed to create log dir {:?}: {}", dir, e);
        return;
    }
    if let Some(parent) = dir.parent() {
        chown_to_user(parent);
    }
    chown_to_user(&dir);

    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let path = dir.join(format!("session-{}.txt", stamp));

    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open log file {:?}: {}", path, e);
            return;
        }
    };
    chown_to_user(&path);

    let _ = LOG_FILE.set(Mutex::new(Some(file)));
    let _ = LOG_PATH.set(path);
}

pub fn log_at(level: LogLevel, file: &str, line: u32, message: &str) {
    let Some(cell) = LOG_FILE.get() else {
        return;
    };
    let Ok(mut guard) = cell.lock() else {
        return;
    };
    let Some(file_handle) = guard.as_mut() else {
        return;
    };

    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!(
        "{} {:<5} {}:{} {}\n",
        now,
        level.as_str(),
        file,
        line,
        message
    );
    let _ = file_handle.write_all(line.as_bytes());
    let _ = file_handle.flush();
}

pub fn current_log_path() -> Option<PathBuf> {
    return LOG_PATH.get().cloned();
}

pub fn list_logs() -> Vec<LogFile> {
    let Some(dir) = logs_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str());
        if ext != Some("txt") && ext != Some("log") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ts_part) = stem.strip_prefix("session-") else {
            continue;
        };
        let Ok(naive) = NaiveDateTime::parse_from_str(ts_part, "%Y-%m-%d_%H-%M-%S") else {
            continue;
        };
        let Some(local) = Local.from_local_datetime(&naive).single() else {
            continue;
        };
        out.push(LogFile {
            path,
            started_at: local,
        });
    }

    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    return out;
}

pub fn cleanup_old_logs(retention_days: u32) {
    if retention_days == 0 {
        return;
    }
    let cutoff: NaiveDate =
        (Local::now() - chrono::Duration::days(retention_days as i64)).date_naive();
    for log in list_logs() {
        if log.started_at.date_naive() < cutoff {
            let _ = fs::remove_file(&log.path);
        }
    }
}

pub fn logs_dir() -> Option<PathBuf> {
    let base = if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(data_home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local").join("share")
    } else {
        return None;
    };
    return Some(base.join("arch-update-manager").join("logs"));
}

pub fn open_logs_folder() {
    let Some(dir) = logs_dir() else {
        return;
    };
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
        chown_to_user(&dir);
    }
    crate::helpers::elevated::spawn_as_user_or_root("xdg-open", &[&dir.to_string_lossy()]);
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::helpers::logger::log_at(
            $crate::models::log_level::LogLevel::Info,
            file!(),
            line!(),
            &format!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::helpers::logger::log_at(
            $crate::models::log_level::LogLevel::Warn,
            file!(),
            line!(),
            &format!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::helpers::logger::log_at(
            $crate::models::log_level::LogLevel::Error,
            file!(),
            line!(),
            &format!($($arg)*),
        )
    };
}
