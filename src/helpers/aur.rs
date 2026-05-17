use crate::{
    constants::AUR_NAME,
    helpers::elevated::get_original_user,
    helpers::settings::{get_effective_aur_helper, load_settings},
    models::{aur_managers::AurManagers, package_update::PackageUpdate},
};
use anyhow::{Context, Result};
use std::process::Command;

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
        AurManagers::PamacCli,
    ];

    for helper in &helpers {
        if is_command_available(helper.command()) {
            return Some(helper.clone());
        }
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
    return parse_aur_updates(&stdout, &helper);
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

fn parse_standard_aur_line(line: &str) -> Result<Option<PackageUpdate>> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() >= 4 && parts[parts.len() - 2] == "->" {
        let package_name = parts[0].to_string();
        let current_version = parts[1].to_string();
        let new_version = parts[parts.len() - 1].to_string();

        return Ok(Some(PackageUpdate {
            repository: AUR_NAME.to_string(),
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
        }));
    }

    return Ok(None);
}

fn parse_pamac_line(line: &str) -> Result<Option<PackageUpdate>> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() >= 3 {
        let package_name = parts[0].to_string();
        let current_version = parts[1].to_string();
        let new_version = parts[2].to_string();

        return Ok(Some(PackageUpdate {
            repository: AUR_NAME.to_string(),
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
        }));
    }

    return Ok(None);
}
