use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::tray_state::state_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnoozeState {
    pub until: DateTime<Utc>,
}

pub fn snooze_file() -> Option<PathBuf> {
    return state_dir().map(|d| d.join("snooze.json"));
}
