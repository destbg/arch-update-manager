use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Duration, Utc};

use crate::helpers::elevated::chown_to_user;
use crate::helpers::tray_state::state_dir;
use crate::models::snooze_state::SnoozeState;

pub fn snooze_file() -> Option<PathBuf> {
    return state_dir().map(|d| d.join("snooze.json"));
}

pub fn current_snooze_until() -> Option<DateTime<Utc>> {
    let state = read_snooze_state()?;
    if state.until > Utc::now() {
        return Some(state.until);
    }
    return None;
}

pub fn read_snooze_state() -> Option<SnoozeState> {
    let path = snooze_file()?;
    let content = fs::read_to_string(&path).ok()?;
    return serde_json::from_str(&content).ok();
}

pub fn set_snooze(hours: u32) -> Result<DateTime<Utc>> {
    if hours == 0 {
        return Err(anyhow!("Snooze duration must be at least 1 hour"));
    }
    let until = Utc::now() + Duration::hours(hours as i64);
    let state = SnoozeState { until };

    let dir = state_dir().ok_or_else(|| anyhow!("Could not determine state directory"))?;
    fs::create_dir_all(&dir).context("Failed to create state directory")?;
    chown_to_user(&dir);

    let path = snooze_file().ok_or_else(|| anyhow!("Could not determine snooze file path"))?;
    let content = serde_json::to_string_pretty(&state).context("Failed to serialize snooze")?;
    fs::write(&path, content).context("Failed to write snooze file")?;
    chown_to_user(&path);
    return Ok(until);
}

pub fn clear_snooze() -> Result<()> {
    let Some(path) = snooze_file() else {
        return Ok(());
    };
    return match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::Error::from(e).context("Failed to remove snooze file")),
    };
}
