use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::helpers::elevated::chown_to_user;
use crate::helpers::tray_state::state_dir;

fn maintainers_file() -> Option<PathBuf> {
    return state_dir().map(|d| d.join("aur_maintainers.json"));
}

pub fn read_maintainers() -> HashMap<String, String> {
    let Some(path) = maintainers_file() else {
        return HashMap::new();
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    return serde_json::from_str(&content).unwrap_or_default();
}

pub fn write_maintainers(map: &HashMap<String, String>) {
    let Some(dir) = state_dir() else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    chown_to_user(&dir);

    let Some(path) = maintainers_file() else {
        return;
    };
    let Ok(content) = serde_json::to_string_pretty(map) else {
        return;
    };
    if fs::write(&path, content).is_ok() {
        chown_to_user(&path);
    }
}
