use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::helpers::elevated::get_original_user;
use crate::models::flatpak_installation::FlatpakInstallation;
use crate::models::installed_flatpak::InstalledFlatpak;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;

const INSTALLATIONS: [FlatpakInstallation; 2] =
    [FlatpakInstallation::User, FlatpakInstallation::System];

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

    let mask = get_flatpak_mask();
    let installed = get_installed_flatpaks();
    let appstream_handler = has_appstream_handler();

    let mut updates = Vec::new();

    for installation in INSTALLATIONS {
        let _ = flatpak_command(installation)
            .args(["update", "--appstream", installation.flag()])
            .output();

        let output = flatpak_command(installation)
            .args(&[
                "remote-ls",
                "--updates",
                "--cached",
                installation.flag(),
                "--columns=application,version,name,origin,download-size",
            ])
            .output()
            .context("Failed to run flatpak remote-ls")?;

        if !output.status.success() {
            continue;
        }

        let text = String::from_utf8_lossy(&output.stdout);

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

            let key = (app_id.clone(), installation);

            let new_version = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            let display_name = parts
                .get(2)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| installed.get(&key).map(|i| i.name.clone()))
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
                .get(&key)
                .map(|i| i.version.clone())
                .unwrap_or_default();

            let url = build_flatpak_url(&origin, &app_id, appstream_handler);

            let new_permissions = get_new_permissions(&app_id, &origin, installation);

            let scope = match installation {
                FlatpakInstallation::User => "user",
                FlatpakInstallation::System => "system",
            };

            updates.push(PackageUpdate {
                source: PackageSource::Flatpak,
                repository: PackageSource::Flatpak.label().to_string(),
                selected: true,
                name: app_id.clone(),
                description: format!("Flatpak application ({}): {}", scope, display_name),
                current_version,
                new_version,
                size: download_size,
                url,
                build_date: None,
                first_submitted: None,
                out_of_date: None,
                orphaned: false,
                maintainer: None,
                previous_maintainer: None,
                num_votes: None,
                popularity: None,
                security_severity: None,
                security_issues: Vec::new(),
                new_permissions,
                extra_dependencies: Vec::new(),
                pkgbuild_needs_review: false,
                aur_scan_findings: Vec::new(),
                flatpak_installation: Some(installation),
                appimage_path: None,
            });
        }
    }

    return Ok(updates);
}

pub fn get_unused_flatpak_runtimes() -> Result<Vec<(String, FlatpakInstallation)>> {
    if !is_flatpak_available() {
        return Ok(Vec::new());
    }

    let mut refs = Vec::new();

    for installation in INSTALLATIONS {
        let mut child = flatpak_command(installation)
            .args(&["uninstall", "--unused", installation.flag()])
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

        for line in text.lines() {
            let trimmed = line.trim_start();
            if !trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                continue;
            }

            for token in trimmed.split_whitespace().skip(1) {
                if token.contains('.') && !token.starts_with('[') {
                    refs.push((token.to_string(), installation));
                    break;
                }
            }
        }
    }

    refs.sort();
    refs.dedup();

    return Ok(refs);
}

pub fn build_flatpak_uninstall_command(refs: &[(String, FlatpakInstallation)]) -> Option<String> {
    return build_flatpak_action_command(refs, "uninstall");
}

pub fn build_flatpak_update_command(packages: &[&PackageUpdate]) -> Option<String> {
    let refs: Vec<(String, FlatpakInstallation)> = packages
        .iter()
        .filter_map(|p| p.flatpak_installation.map(|inst| (p.name.clone(), inst)))
        .collect();
    return build_flatpak_action_command(&refs, "update");
}

fn flatpak_command(installation: FlatpakInstallation) -> Command {
    if installation == FlatpakInstallation::User {
        if let Some(user) = get_original_user() {
            let mut cmd = Command::new("sudo");
            cmd.args(["-u", &user, "flatpak"]);
            return cmd;
        }
    }
    return Command::new("flatpak");
}

fn flatpak_command_prefix(installation: FlatpakInstallation) -> String {
    if installation == FlatpakInstallation::User {
        if let Some(user) = get_original_user() {
            if let Ok(quoted) = shlex::try_quote(&user) {
                return format!("sudo -u {} flatpak", quoted);
            }
        }
    }
    return "flatpak".to_string();
}

fn build_flatpak_action_command(
    refs: &[(String, FlatpakInstallation)],
    action: &str,
) -> Option<String> {
    if refs.is_empty() {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    for installation in INSTALLATIONS {
        let ids: Vec<String> = refs
            .iter()
            .filter(|(_, inst)| *inst == installation)
            .filter_map(|(id, _)| shlex::try_quote(id).ok().map(|c| c.into_owned()))
            .collect();

        if ids.is_empty() {
            continue;
        }

        parts.push(format!(
            "{} {} {} -y {}",
            flatpak_command_prefix(installation),
            action,
            installation.flag(),
            ids.join(" ")
        ));
    }

    if parts.is_empty() {
        return None;
    }

    return Some(parts.join(" && "));
}

fn get_new_permissions(
    app_id: &str,
    origin: &str,
    installation: FlatpakInstallation,
) -> Vec<String> {
    if origin.is_empty() {
        return Vec::new();
    }

    let installed = flatpak_command(installation)
        .args(["info", installation.flag(), "-m", app_id])
        .output();
    let installed = match installed {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return Vec::new(),
    };

    let remote = flatpak_command(installation)
        .args(["remote-info", installation.flag(), "-m", origin, app_id])
        .output();
    let remote = match remote {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return Vec::new(),
    };

    let installed_perms = parse_context_permissions(&installed);
    let remote_perms = parse_context_permissions(&remote);
    return remote_perms.difference(&installed_perms).cloned().collect();
}

fn parse_context_permissions(metadata: &str) -> std::collections::BTreeSet<String> {
    let mut perms = std::collections::BTreeSet::new();
    let mut in_context = false;

    for line in metadata.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_context = line == "[Context]";
            continue;
        }
        if !in_context || line.is_empty() {
            continue;
        }
        let Some((category, values)) = line.split_once('=') else {
            continue;
        };
        let category = category.trim();
        for value in values.split(';') {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            perms.insert(format!("{}: {}", category, value));
        }
    }

    return perms;
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
    let mut cmd = if let Some(user) = get_original_user() {
        let mut c = Command::new("sudo");
        c.args(["-u", &user, "xdg-mime"]);
        c
    } else {
        Command::new("xdg-mime")
    };

    let output = cmd
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
    let mut patterns = Vec::new();

    for installation in INSTALLATIONS {
        let output = flatpak_command(installation)
            .args(["mask", installation.flag()])
            .output();
        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => continue,
        };

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                patterns.push(trimmed.to_string());
            }
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

fn get_installed_flatpaks()
-> std::collections::HashMap<(String, FlatpakInstallation), InstalledFlatpak> {
    use std::collections::HashMap;
    let mut map = HashMap::new();

    for installation in INSTALLATIONS {
        let output = flatpak_command(installation)
            .args(&[
                "list",
                installation.flag(),
                "--columns=application,name,version",
            ])
            .output();
        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => continue,
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
                map.insert(
                    (app_id, installation),
                    InstalledFlatpak {
                        name,
                        version,
                        installation,
                    },
                );
            }
        }
    }

    return map;
}
