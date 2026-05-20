use std::fs;
use std::path::Path;

const POWER_SUPPLY_DIR: &str = "/sys/class/power_supply";

pub fn is_on_battery() -> bool {
    let Ok(entries) = fs::read_dir(POWER_SUPPLY_DIR) else {
        return false;
    };

    let mut has_battery = false;
    let mut mains_paths: Vec<_> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let kind = read_type(&path);
        match kind.as_deref() {
            Some("Battery") => has_battery = true,
            Some("Mains") => mains_paths.push(path),
            _ => {}
        }
    }

    if !has_battery {
        return false;
    }

    for path in &mains_paths {
        if read_online(path) == Some(true) {
            return false;
        }
    }
    return true;
}

fn read_type(dir: &Path) -> Option<String> {
    let content = fs::read_to_string(dir.join("type")).ok()?;
    return Some(content.trim().to_string());
}

fn read_online(dir: &Path) -> Option<bool> {
    let content = fs::read_to_string(dir.join("online")).ok()?;
    return Some(content.trim() == "1");
}
