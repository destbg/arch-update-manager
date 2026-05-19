use std::collections::HashSet;
use std::fs;
use std::process::Command;

pub fn get_desktop_app_packages() -> HashSet<String> {
    let output = match Command::new("pacman").arg("-Ql").output() {
        Ok(o) if o.status.success() => o,
        _ => return HashSet::new(),
    };
    let mut packages: HashSet<String> = HashSet::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let mut parts = line.splitn(2, ' ');
        let pkg = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let file = match parts.next() {
            Some(s) => s.trim(),
            None => continue,
        };
        if !file.ends_with(".desktop") {
            continue;
        }
        if !file.starts_with("/usr/share/applications/") {
            continue;
        }
        if is_hidden_desktop_file(file) {
            continue;
        }
        packages.insert(pkg.to_string());
    }
    return packages;
}

fn is_hidden_desktop_file(path: &str) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let mut in_main_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_main_section = trimmed == "[Desktop Entry]";
            continue;
        }
        if !in_main_section {
            continue;
        }
        let normalized = trimmed.replace(' ', "").to_lowercase();
        if normalized == "nodisplay=true" || normalized == "hidden=true" {
            return true;
        }
    }
    return false;
}
