use alpm::{Alpm, SigLevel};
use anyhow::{Context, Result};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::process::Command;
use std::{error, fmt};

use crate::helpers::appimage::get_appimage_updates;
use crate::helpers::aur::get_aur_updates;
use crate::helpers::flatpak::get_flatpak_updates;
use crate::helpers::security::enrich_with_security;
use crate::helpers::settings::load_settings;
use crate::models::package_info::PackageInfo;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;
use crate::models::update_error::UpdateError;

impl Display for UpdateError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        return match self {
            UpdateError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            UpdateError::IoError(msg) => write!(f, "IO error: {}", msg),
            UpdateError::SyncFailed(msg) => write!(f, "Database sync failed: {}", msg),
        };
    }
}

impl error::Error for UpdateError {}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        return UpdateError::IoError(error.to_string());
    }
}

impl From<anyhow::Error> for UpdateError {
    fn from(error: anyhow::Error) -> Self {
        return UpdateError::IoError(error.to_string());
    }
}

pub fn get_package_updates() -> Result<Vec<PackageUpdate>, UpdateError> {
    let sync_output = Command::new("sudo")
        .args(&["pacman", "-Sy"])
        .output()
        .map_err(|e| {
            UpdateError::CommandFailed(format!("Failed to sync package databases: {}", e))
        })?;

    if !sync_output.status.success() {
        let stderr = String::from_utf8_lossy(&sync_output.stderr);
        return Err(UpdateError::SyncFailed(stderr.to_string()));
    }

    let output = Command::new("pacman")
        .args(&["-Qu"])
        .output()
        .context("Failed to run pacman -Qu")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stderr.trim().is_empty() {
            return Err(UpdateError::CommandFailed(format!(
                "pacman -Qu failed: {}",
                if !stderr.is_empty() {
                    &stderr
                } else {
                    "Exit code 1 with no output"
                }
            )));
        }
    }

    let update_list = String::from_utf8_lossy(&output.stdout);

    let re = Regex::new(r"^(\S+)\s+(\S+)\s+->\s+(\S+)").unwrap();
    let mut package_updates = Vec::new();

    for line in update_list.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Some(captures) = re.captures(line) {
            let package_name = captures[1].to_string();
            let current_version = captures[2].to_string();
            let new_version = captures[3].to_string();

            package_updates.push((package_name, current_version, new_version));
        } else {
            eprintln!("Warning: Could not parse update line: {}", line);
        }
    }

    let mut updates = Vec::new();

    if !package_updates.is_empty() {
        let package_names: Vec<&str> = package_updates
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect();
        let (package_info_map, repo_sizes_map) = get_batch_repository_info(&package_names)?;
        let installed_sizes_map = get_batch_installed_sizes(&package_names)?;
        let build_dates_map = get_build_dates(&package_names);

        for (package_name, current_version, new_version) in package_updates {
            let (description, repository, url) =
                if let Some(info) = package_info_map.get(&package_name) {
                    (
                        info.description.clone(),
                        info.repository.clone(),
                        info.url.clone(),
                    )
                } else {
                    (
                        "No description available".to_string(),
                        "Unknown".to_string(),
                        None,
                    )
                };

            let current_size = installed_sizes_map
                .get(&package_name)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            let new_size = repo_sizes_map
                .get(&package_name)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            let size = calculate_size_difference(&current_size, &new_size);

            let build_date = build_dates_map.get(&package_name).copied();

            updates.push(PackageUpdate {
                source: PackageSource::Official,
                name: package_name,
                new_version,
                current_version,
                description,
                repository,
                selected: true,
                size,
                url,
                build_date,
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
            });
        }

        let new_deps_map = compute_new_dependencies(&package_info_map);
        for update in &mut updates {
            if let Some(deps) = new_deps_map.get(&update.name) {
                update.extra_dependencies = deps.clone();
            }
        }
    }

    let settings = load_settings();
    if settings.enable_aur_support {
        match get_aur_updates() {
            Ok(mut aur_updates) => {
                updates.append(&mut aur_updates);
            }
            Err(e) => {
                eprintln!("Warning: Failed to get AUR updates: {}", e);
            }
        }
    }

    if settings.enable_flatpak_support {
        match get_flatpak_updates() {
            Ok(mut flatpak_updates) => {
                updates.append(&mut flatpak_updates);
            }
            Err(e) => {
                eprintln!("Warning: Failed to get Flatpak updates: {}", e);
            }
        }
    }

    if settings.enable_appimage_support {
        match get_appimage_updates() {
            Ok(mut appimage_updates) => {
                updates.append(&mut appimage_updates);
            }
            Err(e) => {
                eprintln!("Warning: Failed to get AppImage updates: {}", e);
            }
        }
    }

    enrich_with_security(&mut updates);

    updates.sort_by(|a, b| {
        let a_is_core = a.repository.contains("core");
        let b_is_core = b.repository.contains("core");

        return match (a_is_core, b_is_core) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => match a.repository.cmp(&b.repository) {
                Ordering::Equal => a.name.cmp(&b.name),
                other => other,
            },
        };
    });

    return Ok(updates);
}

fn get_batch_repository_info(
    package_names: &[&str],
) -> Result<(HashMap<String, PackageInfo>, HashMap<String, String>), UpdateError> {
    if package_names.is_empty() {
        return Ok((HashMap::new(), HashMap::new()));
    }

    let mut args = vec!["-Si"];
    args.extend(package_names.iter());

    let output = Command::new("pacman")
        .args(&args)
        .output()
        .context("Failed to get batch package info")?;

    let info = String::from_utf8_lossy(&output.stdout);
    let mut package_info_map = HashMap::new();
    let mut repo_sizes_map = HashMap::new();

    let mut current_package = None;
    let mut current_description = "No description available".to_string();
    let mut current_repository = "Unknown".to_string();
    let mut current_url: Option<String> = None;
    let mut current_depends: Vec<String> = Vec::new();

    for line in info.lines() {
        let line = line.trim();

        if line.starts_with("Repository") {
            current_repository = extract_field_value(line);
        } else if line.starts_with("Name") {
            let package_name = extract_field_value(line);

            if !package_info_map.contains_key(&package_name) {
                current_package = Some(package_name);
                current_description = "No description available".to_string();
                current_url = None;
                current_depends = Vec::new();
            } else {
                current_package = None;
            }
        } else if line.starts_with("Description") {
            if current_package.is_some() {
                current_description = extract_field_value(line);
            }
        } else if line.starts_with("URL") {
            if current_package.is_some() {
                let value = extract_field_value(line);
                if !value.is_empty() && value != "Unknown" && value != "None" {
                    current_url = Some(value);
                }
            }
        } else if line.starts_with("Depends On") {
            if current_package.is_some() {
                let value = extract_field_value(line);
                if value != "None" {
                    current_depends = value.split_whitespace().map(|s| s.to_string()).collect();
                }
            }
        } else if line.starts_with("Installed Size") {
            if let Some(ref name) = current_package {
                if !repo_sizes_map.contains_key(name) {
                    repo_sizes_map.insert(name.clone(), extract_field_value(line));
                }
            }
        } else if line.is_empty() && current_package.is_some() {
            if let Some(name) = current_package.take() {
                package_info_map.insert(
                    name,
                    PackageInfo {
                        description: current_description.clone(),
                        repository: current_repository.clone(),
                        url: current_url.clone(),
                        depends: std::mem::take(&mut current_depends),
                    },
                );
                current_url = None;
            }
        }
    }

    if let Some(name) = current_package {
        if !package_info_map.contains_key(&name) {
            package_info_map.insert(
                name,
                PackageInfo {
                    description: current_description,
                    repository: current_repository,
                    url: current_url,
                    depends: current_depends,
                },
            );
        }
    }

    return Ok((package_info_map, repo_sizes_map));
}

fn compute_new_dependencies(
    info_map: &HashMap<String, PackageInfo>,
) -> HashMap<String, Vec<String>> {
    use std::collections::HashSet;

    let mut all_names: HashSet<String> = HashSet::new();
    for info in info_map.values() {
        for dep in &info.depends {
            all_names.insert(strip_dep_version(dep).to_string());
        }
    }
    if all_names.is_empty() {
        return HashMap::new();
    }

    let names: Vec<&str> = all_names.iter().map(|s| s.as_str()).collect();
    let unsatisfied = deptest_unsatisfied(&names);
    if unsatisfied.is_empty() {
        return HashMap::new();
    }

    let mut result = HashMap::new();
    for (name, info) in info_map {
        let mut new_deps: Vec<String> = info
            .depends
            .iter()
            .map(|d| strip_dep_version(d).to_string())
            .filter(|n| unsatisfied.contains(n))
            .collect();
        new_deps.sort();
        new_deps.dedup();
        if !new_deps.is_empty() {
            result.insert(name.clone(), new_deps);
        }
    }
    return result;
}

fn deptest_unsatisfied(names: &[&str]) -> std::collections::HashSet<String> {
    let Ok(output) = Command::new("pacman").arg("-T").args(names).output() else {
        return std::collections::HashSet::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    return text.split_whitespace().map(|s| s.to_string()).collect();
}

fn strip_dep_version(dep: &str) -> &str {
    let end = dep
        .find(|c| c == '<' || c == '>' || c == '=')
        .unwrap_or(dep.len());
    return &dep[..end];
}

fn get_build_dates(package_names: &[&str]) -> HashMap<String, i64> {
    let mut build_dates = HashMap::new();
    if package_names.is_empty() {
        return build_dates;
    }

    let Ok(conf) = pacmanconf::Config::new() else {
        return build_dates;
    };
    let Ok(alpm) = Alpm::new(conf.root_dir.as_str(), conf.db_path.as_str()) else {
        return build_dates;
    };
    for repo in &conf.repos {
        let _ = alpm.register_syncdb(repo.name.as_str(), SigLevel::NONE);
    }

    let wanted: HashSet<&str> = package_names.iter().copied().collect();
    for db in alpm.syncdbs() {
        for name in &wanted {
            if build_dates.contains_key(*name) {
                continue;
            }
            if let Ok(pkg) = db.pkg(*name) {
                build_dates.insert((*name).to_string(), pkg.build_date());
            }
        }
    }

    return build_dates;
}

fn get_batch_installed_sizes(
    package_names: &[&str],
) -> Result<HashMap<String, String>, UpdateError> {
    if package_names.is_empty() {
        return Ok(HashMap::new());
    }

    let mut args = vec!["-Qi"];
    args.extend(package_names.iter());

    let output = Command::new("pacman")
        .args(&args)
        .output()
        .context("Failed to get batch installed package sizes")?;

    let info = String::from_utf8_lossy(&output.stdout);
    let mut sizes_map = HashMap::new();

    let mut current_package = None;

    for line in info.lines() {
        let line = line.trim();

        if line.starts_with("Name") {
            current_package = Some(extract_field_value(line));
        } else if line.starts_with("Installed Size") {
            if let Some(ref name) = current_package {
                sizes_map.insert(name.clone(), extract_field_value(line));
            }
        }
    }

    return Ok(sizes_map);
}

fn calculate_size_difference(current_size_str: &str, new_size_str: &str) -> i64 {
    let current_size = parse_size_string(current_size_str);
    let new_size = parse_size_string(new_size_str);

    if current_size.is_none() || new_size.is_none() {
        return 0;
    }

    let current_bytes = current_size.unwrap();
    let new_bytes = new_size.unwrap();

    if new_bytes > current_bytes {
        let diff_bytes = new_bytes - current_bytes;
        return diff_bytes as i64;
    } else if new_bytes < current_bytes {
        let diff_bytes = current_bytes - new_bytes;
        return -(diff_bytes as i64);
    } else {
        return 0;
    }
}

fn parse_size_string(size_str: &str) -> Option<u64> {
    let size_str = size_str.trim();

    if size_str == "Unknown" || size_str.is_empty() {
        return None;
    }

    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let numeric_part = parts[0].replace(',', ".");
    let value: f64 = numeric_part.parse().ok()?;

    let multiplier = match parts[1] {
        "B" => 1u64,
        "KiB" => 1024u64,
        "MiB" => 1024u64 * 1024u64,
        "GiB" => 1024u64 * 1024u64 * 1024u64,
        "TiB" => 1024u64 * 1024u64 * 1024u64 * 1024u64,
        _ => return None,
    };

    return Some((value * multiplier as f64) as u64);
}

fn extract_field_value(line: &str) -> String {
    if let Some(colon_pos) = line.find(':') {
        return line[colon_pos + 1..].trim().to_string();
    } else {
        return "Unknown".to_string();
    }
}
