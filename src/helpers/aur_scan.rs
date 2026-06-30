use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::helpers::aur::url_encode;
use crate::helpers::aur_pkgbuild::find_clone_dir;
use crate::helpers::elevated::get_original_user;
use crate::helpers::network::http_get;
use crate::models::aur_scan_finding::AurScanFinding;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;

const AUR_SCAN_BIN: &str = "aur-scan";
const PKGBUILD_FETCH_TIMEOUT_SECS: u32 = 10;

static AVAILABLE: OnceLock<bool> = OnceLock::new();

pub fn aur_scan_available() -> bool {
    return *AVAILABLE.get_or_init(|| {
        Command::new("which")
            .arg(AUR_SCAN_BIN)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });
}

pub fn enrich_with_aur_scan(updates: &mut [PackageUpdate]) {
    if !aur_scan_available() {
        return;
    }
    for update in updates.iter_mut() {
        if update.source != PackageSource::Aur {
            continue;
        }
        update.aur_scan_findings = scan_package(&update.name);
    }
}

pub fn scan_package(package: &str) -> Vec<AurScanFinding> {
    if let Some(dir) = find_clone_dir(package) {
        if let Some(findings) = scan_clone_latest(&dir) {
            return findings;
        }
    }
    return scan_remote_pkgbuild(package);
}

fn scan_clone_latest(dir: &Path) -> Option<Vec<AurScanFinding>> {
    let user = get_original_user();
    let dir_str = dir.to_str()?;

    let _ = run_git(user.as_deref(), dir_str, &["fetch", "--quiet"]);

    let output = run_git(user.as_deref(), dir_str, &["archive", "FETCH_HEAD"])?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("pkg");
    let temp = std::env::temp_dir().join(format!(
        "aum-aur-scan-{}-{}",
        std::process::id(),
        sanitize(name)
    ));
    let _ = std::fs::remove_dir_all(&temp);
    if std::fs::create_dir_all(&temp).is_err() {
        return None;
    }

    let findings = extract_and_scan(&output.stdout, &temp);
    let _ = std::fs::remove_dir_all(&temp);
    return findings;
}

fn extract_and_scan(tar_bytes: &[u8], temp: &Path) -> Option<Vec<AurScanFinding>> {
    let tar_path = temp.join("archive.tar");
    std::fs::write(&tar_path, tar_bytes).ok()?;

    let status = Command::new("tar")
        .arg("-xf")
        .arg(&tar_path)
        .arg("-C")
        .arg(temp)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let _ = std::fs::remove_file(&tar_path);

    return Some(run_scan(temp.to_str()?));
}

fn run_git(user: Option<&str>, dir: &str, args: &[&str]) -> Option<std::process::Output> {
    let mut cmd = match user {
        Some(u) => {
            let mut c = Command::new("sudo");
            c.args(["-u", u, "git", "-C", dir]);
            c
        }
        None => {
            let mut c = Command::new("git");
            c.args(["-C", dir]);
            c
        }
    };
    cmd.args(args);
    return cmd.output().ok();
}

fn scan_remote_pkgbuild(package: &str) -> Vec<AurScanFinding> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        url_encode(package)
    );
    let Ok(body) = http_get(&url, PKGBUILD_FETCH_TIMEOUT_SECS) else {
        return Vec::new();
    };

    let dir = std::env::temp_dir().join(format!(
        "aum-aur-scan-{}-{}",
        std::process::id(),
        sanitize(package)
    ));
    if std::fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }

    let pkgbuild = dir.join("PKGBUILD");
    let findings = if std::fs::write(&pkgbuild, body).is_ok() {
        pkgbuild.to_str().map(run_scan).unwrap_or_default()
    } else {
        Vec::new()
    };

    let _ = std::fs::remove_dir_all(&dir);
    return findings;
}

fn sanitize(package: &str) -> String {
    return package
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
}

fn run_scan(path: &str) -> Vec<AurScanFinding> {
    let Ok(output) = Command::new(AUR_SCAN_BIN)
        .args(["scan", "-f", "json", "--no-color", path])
        .output()
    else {
        return Vec::new();
    };
    return parse_findings(&String::from_utf8_lossy(&output.stdout));
}

fn parse_findings(json: &str) -> Vec<AurScanFinding> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(items) = value.get("findings").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut findings = Vec::new();
    for item in items {
        let location = item.get("location");
        findings.push(AurScanFinding {
            id: str_field(item, "id"),
            severity: str_field(item, "severity"),
            category: str_field(item, "category"),
            title: str_field(item, "title"),
            description: str_field(item, "description"),
            recommendation: str_field(item, "recommendation"),
            file: opt_str(location, "file"),
            line: location
                .and_then(|l| l.get("line"))
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            snippet: opt_str(location, "snippet"),
        });
    }
    return findings;
}

fn str_field(item: &serde_json::Value, key: &str) -> String {
    return item
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
}

fn opt_str(location: Option<&serde_json::Value>, key: &str) -> Option<String> {
    return location
        .and_then(|l| l.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
}
