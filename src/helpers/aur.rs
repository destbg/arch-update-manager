use crate::{
    helpers::aur_maintainers::{read_maintainers, write_maintainers},
    helpers::aur_pkgbuild::pkgbuild_needs_review,
    helpers::aur_scan::enrich_with_aur_scan,
    helpers::elevated::get_original_user,
    helpers::network::http_get,
    helpers::settings::{get_effective_aur_helper, load_settings},
    models::{
        aur_info::AurInfo, aur_managers::AurManagers, package_source::PackageSource,
        package_update::PackageUpdate, shelly_update::ShellyUpdate,
    },
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;

const AUR_RPC_TIMEOUT_SECS: u32 = 5;

pub fn detect_aur_helper() -> Option<AurManagers> {
    let settings = load_settings();

    if let Some(helper_name) = get_effective_aur_helper(&settings) {
        if let Some(helper) = AurManagers::from_command(&helper_name) {
            return Some(helper);
        }
    }

    let helpers = [
        AurManagers::Yay,
        AurManagers::Paru,
        AurManagers::Trizen,
        AurManagers::Pikaur,
        AurManagers::Shelly,
        AurManagers::PamacCli,
    ];

    for helper in &helpers {
        if !is_command_available(helper.command()) {
            continue;
        }
        if matches!(helper, AurManagers::PamacCli) && !pamac_supports_aur() {
            continue;
        }
        if matches!(helper, AurManagers::Shelly) && !shelly_supports_aur() {
            continue;
        }
        return Some(helper.clone());
    }

    return None;
}

pub fn is_command_available(command: &str) -> bool {
    return Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

pub fn shelly_supports_aur() -> bool {
    let Ok(output) = Command::new("shelly")
        .args(["config", "get", "AurEnabled"])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    return String::from_utf8_lossy(&output.stdout)
        .trim()
        .eq_ignore_ascii_case("true");
}

pub fn pamac_supports_aur() -> bool {
    let Ok(output) = Command::new("pamac").args(["list", "--help"]).output() else {
        return false;
    };
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    return combined.contains("--aur") || combined.contains(" -a,");
}

pub fn get_aur_updates() -> Result<Vec<PackageUpdate>> {
    let Some(helper) = detect_aur_helper() else {
        return Ok(Vec::new());
    };

    let settings = load_settings();
    let mut args = helper.update_check_args();
    if settings.enable_devel_aur {
        args.extend(helper.devel_args());
    }

    let output = Command::new(helper.command())
        .args(&args)
        .output()
        .context(format!(
            "Failed to run {} for AUR updates",
            helper.command()
        ))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("nothing to do")
            || stderr.contains("no packages")
            || output.stdout.is_empty()
        {
            return Ok(Vec::new());
        }
        return Err(anyhow::anyhow!("AUR helper failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut updates = parse_aur_updates(&stdout, &helper)?;
    enrich_with_aur_info(&mut updates);
    for update in &mut updates {
        update.pkgbuild_needs_review = pkgbuild_needs_review(&update.name);
    }
    enrich_with_aur_scan(&mut updates);
    return Ok(updates);
}

pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    return out;
}

fn enrich_with_aur_info(updates: &mut [PackageUpdate]) {
    if updates.is_empty() {
        return;
    }

    let names: Vec<&str> = updates.iter().map(|u| u.name.as_str()).collect();
    let info_map = fetch_aur_info(&names);
    if info_map.is_empty() {
        return;
    }

    let mut known_maintainers = read_maintainers();
    let mut maintainers_changed = false;

    for update in updates.iter_mut() {
        let Some(info) = info_map.get(&update.name) else {
            continue;
        };

        if let Some(description) = &info.description {
            if !description.is_empty() {
                update.description = description.clone();
            }
        }
        if let Some(url) = &info.url {
            if !url.is_empty() {
                update.url = Some(url.clone());
            }
        }
        update.build_date = info.last_modified;
        update.first_submitted = info.first_submitted;
        update.out_of_date = info.out_of_date;
        update.orphaned = info.maintainer.is_none();
        update.maintainer = info.maintainer.clone();
        update.num_votes = info.num_votes;
        update.popularity = info.popularity;

        if let Some(current) = &info.maintainer {
            match known_maintainers.get(&update.name) {
                Some(previous) if previous != current => {
                    update.previous_maintainer = Some(previous.clone());
                }
                None => {
                    known_maintainers.insert(update.name.clone(), current.clone());
                    maintainers_changed = true;
                }
                _ => {}
            }
        }
    }

    if maintainers_changed {
        write_maintainers(&known_maintainers);
    }
}

fn fetch_aur_info(names: &[&str]) -> HashMap<String, AurInfo> {
    let mut map = HashMap::new();
    if names.is_empty() {
        return map;
    }

    let url = aur_rpc_info_url(names);
    let Ok(body) = http_get(&url, AUR_RPC_TIMEOUT_SECS) else {
        return map;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
        return map;
    };
    let Some(results) = json.get("results").and_then(|r| r.as_array()) else {
        return map;
    };

    for entry in results {
        let Some(name) = entry.get("Name").and_then(|n| n.as_str()) else {
            continue;
        };
        let str_field = |key: &str| {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        map.insert(
            name.to_string(),
            AurInfo {
                description: str_field("Description"),
                url: str_field("URL"),
                last_modified: entry.get("LastModified").and_then(|v| v.as_i64()),
                first_submitted: entry.get("FirstSubmitted").and_then(|v| v.as_i64()),
                out_of_date: entry.get("OutOfDate").and_then(|v| v.as_i64()),
                maintainer: str_field("Maintainer"),
                num_votes: entry.get("NumVotes").and_then(|v| v.as_i64()),
                popularity: entry.get("Popularity").and_then(|v| v.as_f64()),
            },
        );
    }

    return map;
}

fn aur_rpc_info_url(names: &[&str]) -> String {
    let mut url = String::from("https://aur.archlinux.org/rpc/v5/info?");
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            url.push('&');
        }

        url.push_str("arg%5B%5D=");
        url.push_str(&url_encode(name));
    }
    return url;
}

pub fn install_aur_packages(packages: Vec<String>) -> Result<Vec<String>> {
    let Some(helper) = detect_aur_helper() else {
        return Err(anyhow::anyhow!("No AUR helper available for installation"));
    };

    let settings = load_settings();
    let mut args = helper.install_args().to_vec();
    if settings.enable_devel_aur {
        args.extend(helper.devel_args());
    }

    for package in &packages {
        args.push(package);
    }

    let original_user = get_original_user();

    if let Some(user) = original_user {
        let mut command_parts = vec![
            "sudo".to_string(),
            "-u".to_string(),
            user,
            helper.command().to_string(),
        ];
        command_parts.extend(args.into_iter().map(|s| s.to_string()));
        return Ok(command_parts);
    } else {
        let mut command_parts = vec![helper.command().to_string()];
        command_parts.extend(args.into_iter().map(|s| s.to_string()));
        return Ok(command_parts);
    }
}

fn parse_aur_updates(output: &str, helper: &AurManagers) -> Result<Vec<PackageUpdate>> {
    if matches!(helper, AurManagers::Shelly) {
        return parse_shelly_updates(output);
    }

    let mut updates = Vec::new();

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let package_update = match helper {
            AurManagers::PamacCli => parse_pamac_line(line)?,
            _ => parse_standard_aur_line(line)?,
        };

        if let Some(update) = package_update {
            updates.push(update);
        }
    }

    return Ok(updates);
}

fn parse_shelly_updates(output: &str) -> Result<Vec<PackageUpdate>> {
    let trimmed = output.trim_start_matches('\u{FEFF}').trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let entries: Vec<ShellyUpdate> =
        serde_json::from_str(trimmed).context("Failed to parse shelly aur list-updates JSON")?;

    return Ok(entries
        .into_iter()
        .map(|e| PackageUpdate {
            source: PackageSource::Aur,
            repository: PackageSource::Aur.label().to_string(),
            selected: true,
            description: format!("AUR package: {}", e.name),
            url: Some(format!("https://aur.archlinux.org/packages/{}", e.name)),
            size: e.download_size.max(0),
            current_version: e.current_version,
            new_version: e.new_version,
            name: e.name,
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
            new_permissions: Vec::new(),
            extra_dependencies: Vec::new(),
            pkgbuild_needs_review: false,
            aur_scan_findings: Vec::new(),
            flatpak_installation: None,
            appimage_path: None,
        })
        .collect());
}

fn parse_standard_aur_line(line: &str) -> Result<Option<PackageUpdate>> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() >= 4 && parts[parts.len() - 2] == "->" {
        let package_name = parts[0].to_string();
        let current_version = parts[1].to_string();
        let new_version = parts[parts.len() - 1].to_string();

        return Ok(Some(PackageUpdate {
            source: PackageSource::Aur,
            repository: PackageSource::Aur.label().to_string(),
            selected: true,
            name: package_name.clone(),
            description: format!("AUR package: {}", package_name),
            current_version,
            new_version,
            size: 0,
            url: Some(format!(
                "https://aur.archlinux.org/packages/{}",
                package_name
            )),
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
            new_permissions: Vec::new(),
            extra_dependencies: Vec::new(),
            pkgbuild_needs_review: false,
            aur_scan_findings: Vec::new(),
            flatpak_installation: None,
            appimage_path: None,
        }));
    }

    return Ok(None);
}

fn parse_pamac_line(line: &str) -> Result<Option<PackageUpdate>> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() >= 3
        && is_plausible_package_name(parts[0])
        && is_plausible_version(parts[1])
        && is_plausible_version(parts[2])
    {
        let package_name = parts[0].to_string();
        let current_version = parts[1].to_string();
        let new_version = parts[2].to_string();

        return Ok(Some(PackageUpdate {
            source: PackageSource::Aur,
            repository: PackageSource::Aur.label().to_string(),
            selected: true,
            name: package_name.clone(),
            description: format!("AUR package: {}", package_name),
            current_version,
            new_version,
            size: 0,
            url: Some(format!(
                "https://aur.archlinux.org/packages/{}",
                package_name
            )),
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
            new_permissions: Vec::new(),
            extra_dependencies: Vec::new(),
            pkgbuild_needs_review: false,
            aur_scan_findings: Vec::new(),
            flatpak_installation: None,
            appimage_path: None,
        }));
    }

    return Ok(None);
}

fn is_plausible_package_name(s: &str) -> bool {
    if s.is_empty() || s.starts_with('-') {
        return false;
    }
    return s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '.' | '@'));
}

fn is_plausible_version(s: &str) -> bool {
    if s.is_empty() || s.starts_with('-') {
        return false;
    }
    return s
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false);
}
