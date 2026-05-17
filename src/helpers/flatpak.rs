use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::constants::FLATPAK_NAME;
use crate::models::installed_flatpak::InstalledFlatpak;
use crate::models::package_update::PackageUpdate;

pub fn is_flatpak_available() -> bool {
    return Command::new("which")
        .arg("flatpak")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

pub fn get_flatpak_updates() -> Result<Vec<PackageUpdate>> {
    if !is_flatpak_available() {
        return Ok(Vec::new());
    }

    let _ = Command::new("flatpak")
        .args(&["update", "--appstream"])
        .output();

    let mask = get_flatpak_mask();
    let installed = get_installed_flatpaks();
    let appstream_handler = has_appstream_handler();

    let output = Command::new("flatpak")
        .args(&[
            "remote-ls",
            "--updates",
            "--cached",
            "--columns=application,version,name,origin,download-size",
        ])
        .output()
        .context("Failed to run flatpak remote-ls")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut updates = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.is_empty() {
            continue;
        }

        let app_id = parts.first().unwrap_or(&"").trim().to_string();
        if app_id.is_empty() {
            continue;
        }

        if is_masked(&mask, &app_id) {
            continue;
        }

        let new_version = parts
            .get(1)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let display_name = parts
            .get(2)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| installed.get(&app_id).map(|i| i.name.clone()))
            .unwrap_or_else(|| app_id.clone());

        let origin = parts
            .get(3)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let download_size = parts
            .get(4)
            .and_then(|s| parse_flatpak_size(s.trim()))
            .unwrap_or(0);

        let current_version = installed
            .get(&app_id)
            .map(|i| i.version.clone())
            .unwrap_or_default();

        let url = build_flatpak_url(&origin, &app_id, appstream_handler);

        updates.push(PackageUpdate {
            repository: FLATPAK_NAME.to_string(),
            selected: true,
            name: app_id.clone(),
            description: format!("Flatpak application: {}", display_name),
            current_version,
            new_version,
            size: download_size,
            url,
        });
    }

    return Ok(updates);
}

pub fn get_unused_flatpak_runtimes() -> Result<Vec<String>> {
    if !is_flatpak_available() {
        return Ok(Vec::new());
    }

    let mut child = Command::new("flatpak")
        .args(&["uninstall", "--unused"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn flatpak uninstall --unused")?;

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"n\n");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut refs = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            continue;
        }

        for token in trimmed.split_whitespace().skip(1) {
            if token.contains('.') && !token.starts_with('[') {
                refs.push(token.to_string());
                break;
            }
        }
    }

    refs.sort();
    refs.dedup();

    return Ok(refs);
}

pub fn build_flatpak_uninstall_command(app_ids: &[String]) -> Option<String> {
    if app_ids.is_empty() {
        return None;
    }

    let quoted: Vec<String> = app_ids
        .iter()
        .filter_map(|p| shlex::try_quote(p).ok().map(|c| c.into_owned()))
        .collect();

    if quoted.is_empty() {
        return None;
    }

    return Some(format!("flatpak uninstall -y {}", quoted.join(" ")));
}

pub fn build_flatpak_update_command(app_ids: &[String]) -> Option<String> {
    if app_ids.is_empty() {
        return None;
    }

    let quoted: Vec<String> = app_ids
        .iter()
        .filter_map(|p| shlex::try_quote(p).ok().map(|c| c.into_owned()))
        .collect();

    if quoted.is_empty() {
        return None;
    }

    return Some(format!("flatpak update -y {}", quoted.join(" ")));
}

fn build_flatpak_url(origin: &str, app_id: &str, appstream_handler: bool) -> Option<String> {
    if appstream_handler {
        return Some(format!("appstream://{}", app_id));
    }

    let lower = origin.to_lowercase();
    if lower == "flathub" || lower == "flathub-beta" {
        return Some(format!("https://flathub.org/apps/{}", app_id));
    }
    return None;
}

fn has_appstream_handler() -> bool {
    let output = Command::new("xdg-mime")
        .args(&["query", "default", "x-scheme-handler/appstream"])
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let handler = String::from_utf8_lossy(&output.stdout).trim().to_string();
    return !handler.is_empty();
}

fn parse_flatpak_size(value: &str) -> Option<i64> {
    if value.is_empty() || value == "?" {
        return None;
    }

    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let numeric: f64 = parts[0].replace(',', ".").parse().ok()?;

    let multiplier: f64 = if parts.len() >= 2 {
        match parts[1].to_ascii_lowercase().as_str() {
            "b" | "bytes" => 1.0,
            "kb" => 1000.0,
            "mb" => 1000.0 * 1000.0,
            "gb" => 1000.0 * 1000.0 * 1000.0,
            "tb" => 1000.0 * 1000.0 * 1000.0 * 1000.0,
            "kib" => 1024.0,
            "mib" => 1024.0 * 1024.0,
            "gib" => 1024.0 * 1024.0 * 1024.0,
            "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            _ => return None,
        }
    } else {
        1.0
    };

    let bytes = (numeric * multiplier) as i64;
    if bytes <= 0 {
        return None;
    }
    return Some(bytes);
}

fn get_flatpak_mask() -> Vec<String> {
    let output = Command::new("flatpak").arg("mask").output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut patterns = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            patterns.push(trimmed.to_string());
        }
    }
    return patterns;
}

fn is_masked(mask: &[String], app_id: &str) -> bool {
    for pattern in mask {
        if pattern_matches(pattern, app_id) {
            return true;
        }
    }
    return false;
}

fn pattern_matches(pattern: &str, app_id: &str) -> bool {
    if pattern == app_id {
        return true;
    }

    if !pattern.contains('*') {
        return false;
    }

    let mut p = pattern;
    let mut s = app_id;

    if let Some(prefix) = p.strip_suffix('*') {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest;
            p = "*";
        } else {
            return false;
        }
    }

    if p == "*" {
        return true;
    }

    return p == s;
}

fn get_installed_flatpaks() -> std::collections::HashMap<String, InstalledFlatpak> {
    use std::collections::HashMap;
    let mut map = HashMap::new();

    let output = Command::new("flatpak")
        .args(&["list", "--columns=application,name,version"])
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return map,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() >= 2 {
            let app_id = parts[0].trim().to_string();
            let name = parts[1].trim().to_string();
            let version = parts
                .get(2)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            map.insert(app_id, InstalledFlatpak { name, version });
        }
    }

    return map;
}
