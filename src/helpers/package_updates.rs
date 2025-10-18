use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::process::Command;

use crate::models::package_info::PackageInfo;
use crate::models::package_update::PackageUpdate;
use crate::models::update_error::UpdateError;

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UpdateError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            UpdateError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for UpdateError {}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        UpdateError::IoError(error.to_string())
    }
}

impl From<anyhow::Error> for UpdateError {
    fn from(error: anyhow::Error) -> Self {
        UpdateError::IoError(error.to_string())
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
        eprintln!("Warning: Failed to sync databases: {}", stderr);
    }

    let output = Command::new("pacman")
        .args(&["-Qu"])
        .output()
        .context("Failed to run pacman -Qu")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stderr.contains("no packages to upgrade")
            || stderr.trim().is_empty() && stdout.trim().is_empty()
        {
            return Ok(Vec::new());
        }

        return Err(UpdateError::CommandFailed(format!(
            "pacman -Qu failed: {}",
            if !stderr.is_empty() {
                &stderr
            } else {
                "Exit code 1 with no output"
            }
        )));
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

    if package_updates.is_empty() {
        return Ok(Vec::new());
    }

    let package_names: Vec<&str> = package_updates
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    let (package_info_map, repo_sizes_map) = get_batch_repository_info(&package_names)?;
    let installed_sizes_map = get_batch_installed_sizes(&package_names)?;

    let mut updates = Vec::new();
    for (package_name, current_version, new_version) in package_updates {
        let (description, repository) = if let Some(info) = package_info_map.get(&package_name) {
            (info.description.clone(), info.repository.clone())
        } else {
            (
                "No description available".to_string(),
                "Unknown".to_string(),
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

        updates.push(PackageUpdate {
            name: package_name,
            new_version,
            current_version,
            description,
            repository,
            selected: true,
            size,
        });
    }

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

    for line in info.lines() {
        let line = line.trim();

        if line.starts_with("Repository") {
            current_repository = extract_field_value(line);
        } else if line.starts_with("Name") {
            let package_name = extract_field_value(line);

            if !package_info_map.contains_key(&package_name) {
                current_package = Some(package_name);
                current_description = "No description available".to_string();
            } else {
                current_package = None;
            }
        } else if line.starts_with("Description") {
            if current_package.is_some() {
                current_description = extract_field_value(line);
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
                    },
                );
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
                },
            );
        }
    }

    return Ok((package_info_map, repo_sizes_map));
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
