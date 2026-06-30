use anyhow::Result;
use regex::Regex;
use std::path::Path;
use std::process::Command;

use crate::models::cache_candidates::CacheCandidates;
use crate::models::paccache_dry_result::PaccacheDryResult;
use crate::models::service_restart_outcome::ServiceRestartOutcome;

pub fn get_pacnew_files() -> Result<Vec<String>> {
    let output = Command::new("pacdiff").arg("-o").output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("/etc/") || trimmed == "/etc" {
            continue;
        }
        files.push(trimmed.to_string());
    }

    return Ok(files);
}

pub fn is_meld_available() -> bool {
    return Command::new("which")
        .arg("meld")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

pub fn get_orphan_packages() -> Result<Vec<String>> {
    let output = Command::new("pacman").args(&["-Qtdq"]).output()?;

    if !output.status.success() && !output.stdout.is_empty() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut packages = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            packages.push(trimmed.to_string());
        }
    }

    return Ok(packages);
}

pub fn get_cache_candidates(keep_old: u32, keep_uninstalled: u32) -> Result<CacheCandidates> {
    let mut result = CacheCandidates::default();
    let mut total_bytes: u64 = 0;
    let mut total_unit: Option<String> = None;

    let old = run_paccache_dry(&["-dv", &format!("-k{}", keep_old)])?;
    result.old_count = old.count;
    result.old_packages = old.packages;
    if let Some((bytes, unit)) = old.space {
        total_bytes = total_bytes.saturating_add(bytes);
        total_unit = Some(unit);
    }

    let uninst = run_paccache_dry(&["-dv", "-u", &format!("-k{}", keep_uninstalled)])?;
    result.uninstalled_count = uninst.count;
    result.uninstalled_packages = uninst.packages;
    if let Some((bytes, unit)) = uninst.space {
        total_bytes = total_bytes.saturating_add(bytes);
        if total_unit.is_none() {
            total_unit = Some(unit);
        }
    }

    if total_bytes > 0 {
        result.disk_space = Some(format_bytes(total_bytes));
    } else if let Some(unit) = total_unit {
        result.disk_space = Some(format!("0.00 {}", unit));
    }

    return Ok(result);
}

pub fn get_services_needing_restart() -> Result<Vec<String>> {
    let exclusions = [
        "gdm.service",
        "sddm.service",
        "lightdm.service",
        "lxdm.service",
        "plasmalogin.service",
        "slim.service",
        "xdm.service",
        "greetd.service",
        "nodm.service",
        "ly.service",
        "lemurs.service",
    ];

    let mut args: Vec<&str> = vec!["-F", "-P", "-R"];
    for excluded in &exclusions {
        args.push("-i");
        args.push(excluded);
    }

    let output = Command::new("checkservices").args(&args).output();
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to run checkservices: {}", e);
            return Ok(Vec::new());
        }
    };

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut services = Vec::new();
    for line in combined.lines() {
        if !line.contains(".service") {
            continue;
        }

        if let Some(name) = extract_service_name(line) {
            if !services.contains(&name) {
                services.push(name);
            }
        }
    }

    return Ok(services);
}

pub fn restart_service(service: &str) -> ServiceRestartOutcome {
    let output = Command::new("systemctl")
        .args(&["restart", service])
        .output();

    return match output {
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            ServiceRestartOutcome {
                success: o.status.success(),
                exit_code: o.status.code(),
                stdout,
                stderr,
            }
        }
        Err(e) => ServiceRestartOutcome {
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Could not run systemctl: {}", e),
        },
    };
}

pub fn is_kernel_reboot_pending() -> bool {
    if is_kernel_modules_hook_installed() {
        return false;
    }

    if is_in_container() {
        return false;
    }

    let kernel_release = match Command::new("uname").arg("-r").output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => return false,
    };

    if kernel_release.is_empty() {
        return false;
    }

    let kernel_path = format!("/usr/lib/modules/{}/vmlinuz", kernel_release);
    return !Path::new(&kernel_path).exists();
}

pub fn clean_cache(keep_old: u32, keep_uninstalled: u32) -> Result<()> {
    let keep_old_arg = format!("-k{}", keep_old);
    let old_status = Command::new("paccache")
        .args(["-r", &keep_old_arg])
        .status()?;
    if !old_status.success() {
        return Err(anyhow::anyhow!("paccache failed to remove old packages"));
    }

    let keep_uninstalled_arg = format!("-k{}", keep_uninstalled);
    let uninstalled_status = Command::new("paccache")
        .args(["-r", "-u", &keep_uninstalled_arg])
        .status()?;
    if !uninstalled_status.success() {
        return Err(anyhow::anyhow!(
            "paccache failed to remove uninstalled packages"
        ));
    }

    return Ok(());
}

fn extract_service_name(line: &str) -> Option<String> {
    if let Some(start) = line.find('\'') {
        if let Some(end) = line[start + 1..].find('\'') {
            return Some(line[start + 1..start + 1 + end].to_string());
        }
    }

    for token in line.split_whitespace() {
        if token.ends_with(".service") {
            return Some(token.trim_matches('\'').to_string());
        }
    }

    return None;
}

fn run_paccache_dry(extra_args: &[&str]) -> Result<PaccacheDryResult> {
    let mut args: Vec<&str> = Vec::new();
    args.extend(extra_args);

    let output = Command::new("paccache").args(&args).output()?;

    if !output.status.success() {
        return Ok(PaccacheDryResult {
            count: 0,
            space: None,
            packages: Vec::new(),
        });
    }

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let count_re = Regex::new(r"(\d+)\s+candidate").unwrap();
    let count = count_re
        .captures(&combined)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or(0);

    let space_re = Regex::new(r"disk space saved:\s*(\d+(?:\.\d+)?)\s*([A-Za-z]+)").unwrap();
    let space = space_re.captures(&combined).and_then(|c| {
        let value: f64 = c.get(1)?.as_str().parse().ok()?;
        let unit = c.get(2)?.as_str().to_string();
        let bytes = bytes_from_unit(value, &unit)?;
        return Some((bytes, unit));
    });

    let mut packages = Vec::new();
    for line in combined.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("==>") || trimmed.is_empty() {
            continue;
        }
        if let Some(name) = extract_package_name(trimmed) {
            packages.push(name);
        }
    }

    return Ok(PaccacheDryResult {
        count,
        space,
        packages,
    });
}

fn extract_package_name(line: &str) -> Option<String> {
    let path = line.trim();
    let file_name = match path.rsplit('/').next() {
        Some(name) => name,
        None => path,
    };

    let without_ext = strip_pkg_extension(file_name);
    return Some(without_ext);
}

fn strip_pkg_extension(name: &str) -> String {
    for suffix in [
        ".pkg.tar.zst",
        ".pkg.tar.xz",
        ".pkg.tar.gz",
        ".pkg.tar.bz2",
        ".pkg.tar",
    ] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    return name.to_string();
}

fn bytes_from_unit(value: f64, unit: &str) -> Option<u64> {
    let multiplier = match unit {
        "B" => 1.0,
        "KiB" | "K" => 1024.0,
        "MiB" | "M" => 1024.0 * 1024.0,
        "GiB" | "G" => 1024.0 * 1024.0 * 1024.0,
        "TiB" | "T" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    return Some((value * multiplier) as u64);
}

fn is_kernel_modules_hook_installed() -> bool {
    let output = Command::new("pacman")
        .args(&["-Q", "kernel-modules-hook"])
        .output();
    return matches!(output, Ok(o) if o.status.success());
}

fn is_in_container() -> bool {
    let output = Command::new("systemd-detect-virt")
        .args(&["--container", "--quiet"])
        .status();
    return matches!(output, Ok(s) if s.success());
}

fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut idx = 0;
    let mut value = bytes as f64;
    while value >= 1024.0 && idx + 1 < units.len() {
        value /= 1024.0;
        idx += 1;
    }
    return format!("{:.2} {}", value, units[idx]);
}
