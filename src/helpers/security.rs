use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use alpm::vercmp;

use crate::helpers::elevated::chown_to_user;
use crate::helpers::installed_packages::get_all_installed_packages;
use crate::helpers::network::http_get;
use crate::helpers::tray_state::state_dir;
use crate::models::open_vulnerability::OpenVulnerability;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;
use crate::models::security_fix::SecurityFix;

const SECURITY_URL: &str = "https://security.archlinux.org/all.json";
const SECURITY_TIMEOUT_SECS: u32 = 8;
const CACHE_MAX_AGE: Duration = Duration::from_secs(6 * 3600);

pub fn enrich_with_security(updates: &mut [PackageUpdate]) {
    if updates.is_empty() {
        return;
    }

    let Some(json) = load_tracker_json() else {
        return;
    };
    let fixes = parse_fixes(&json);
    if fixes.is_empty() {
        return;
    }

    for update in updates.iter_mut() {
        if update.source != PackageSource::Official {
            continue;
        }
        let Some(groups) = fixes.get(&update.name) else {
            continue;
        };

        let mut best: Option<(u8, String)> = None;
        let mut issues: Vec<String> = Vec::new();

        for group in groups {
            let was_vulnerable =
                vercmp(update.current_version.as_str(), group.fixed.as_str()) == Ordering::Less;
            let now_fixed =
                vercmp(update.new_version.as_str(), group.fixed.as_str()) != Ordering::Less;
            if !was_vulnerable || !now_fixed {
                continue;
            }

            let rank = severity_rank(&group.severity);
            let take = match &best {
                Some((current, _)) => rank > *current,
                None => true,
            };
            if take {
                best = Some((rank, group.severity.clone()));
            }

            for issue in &group.issues {
                if !issues.contains(issue) {
                    issues.push(issue.clone());
                }
            }
        }

        if let Some((_, severity)) = best {
            update.security_severity = Some(severity);
            update.security_issues = issues;
        }
    }
}

pub fn get_open_vulnerabilities() -> Option<Vec<OpenVulnerability>> {
    let json = load_tracker_json()?;
    let value = serde_json::from_str::<serde_json::Value>(&json).ok()?;
    let groups = value.as_array()?;

    let installed: HashSet<String> = get_all_installed_packages().into_iter().collect();
    let mut per_package: HashMap<String, OpenVulnerability> = HashMap::new();

    for group in groups {
        let status = group.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if !status.eq_ignore_ascii_case("Vulnerable") {
            continue;
        }
        let has_fix = group
            .get("fixed")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if has_fix {
            continue;
        }

        let severity = group
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let vuln_type = group
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let issues: Vec<String> = group
            .get("issues")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| i.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let Some(packages) = group.get("packages").and_then(|v| v.as_array()) else {
            continue;
        };

        for pkg in packages {
            let Some(name) = pkg.as_str() else {
                continue;
            };
            if !installed.contains(name) {
                continue;
            }

            let entry = per_package
                .entry(name.to_string())
                .or_insert_with(|| OpenVulnerability {
                    package: name.to_string(),
                    severity: severity.clone(),
                    issues: Vec::new(),
                    types: Vec::new(),
                });

            if severity_rank(&severity) > severity_rank(&entry.severity) {
                entry.severity = severity.clone();
            }
            for issue in &issues {
                if !entry.issues.contains(issue) {
                    entry.issues.push(issue.clone());
                }
            }
            if !vuln_type.is_empty() && !entry.types.contains(&vuln_type) {
                entry.types.push(vuln_type.clone());
            }
        }
    }

    let mut result: Vec<OpenVulnerability> = per_package.into_values().collect();
    result.sort_by(|a, b| {
        severity_rank(&b.severity)
            .cmp(&severity_rank(&a.severity))
            .then_with(|| a.package.cmp(&b.package))
    });

    return Some(result);
}

fn parse_fixes(json: &str) -> HashMap<String, Vec<SecurityFix>> {
    let mut map: HashMap<String, Vec<SecurityFix>> = HashMap::new();

    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return map;
    };
    let Some(groups) = value.as_array() else {
        return map;
    };

    for group in groups {
        let Some(fixed) = group.get("fixed").and_then(|v| v.as_str()) else {
            continue;
        };
        if fixed.is_empty() {
            continue;
        }

        let severity = group
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let issues: Vec<String> = group
            .get("issues")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| i.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let Some(packages) = group.get("packages").and_then(|v| v.as_array()) else {
            continue;
        };

        for pkg in packages {
            let Some(name) = pkg.as_str() else {
                continue;
            };
            map.entry(name.to_string()).or_default().push(SecurityFix {
                severity: severity.clone(),
                fixed: fixed.to_string(),
                issues: issues.clone(),
            });
        }
    }

    return map;
}

fn severity_rank(severity: &str) -> u8 {
    return match severity.to_ascii_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    };
}

fn cache_file() -> Option<PathBuf> {
    return state_dir().map(|d| d.join("security.json"));
}

fn load_tracker_json() -> Option<String> {
    if let Some(path) = cache_file() {
        if let Ok(meta) = fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = modified.elapsed() {
                    if age < CACHE_MAX_AGE {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if !content.is_empty() {
                                return Some(content);
                            }
                        }
                    }
                }
            }
        }
    }

    let body = http_get(SECURITY_URL, SECURITY_TIMEOUT_SECS).ok()?;
    write_cache(&body);
    return Some(body);
}

fn write_cache(body: &str) {
    let Some(dir) = state_dir() else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    chown_to_user(&dir);

    let Some(path) = cache_file() else {
        return;
    };
    if fs::write(&path, body).is_ok() {
        chown_to_user(&path);
    }
}
