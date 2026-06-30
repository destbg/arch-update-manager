use alpm::{Alpm, PackageValidation, SigLevel, vercmp};
use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::models::package_source::PackageSource;
use crate::models::repo_switch::{RepoSwitch, SwitchKind};

pub fn detect_repo_switches() -> Result<Vec<RepoSwitch>> {
    let pacman_conf = pacmanconf::Config::new().context("failed to read pacman.conf")?;

    let alpm = Alpm::new(pacman_conf.root_dir.as_str(), pacman_conf.db_path.as_str())
        .context("failed to initialize alpm handle")?;

    for repo in &pacman_conf.repos {
        alpm.register_syncdb(repo.name.as_str(), SigLevel::NONE)
            .with_context(|| format!("failed to register syncdb {}", repo.name))?;
    }

    let local_db_dir = PathBuf::from(&pacman_conf.db_path).join("local");
    let sync_repo_names: Vec<String> = pacman_conf.repos.iter().map(|r| r.name.clone()).collect();

    let local = alpm.localdb();
    let mut switches: Vec<RepoSwitch> = Vec::new();

    let mut installed_meta: HashMap<String, String> = HashMap::new();
    for pkg in local.pkgs() {
        installed_meta.insert(pkg.name().to_string(), pkg.version().to_string());
    }

    for pkg in local.pkgs() {
        let name = pkg.name();
        let version_str = pkg.version().to_string();

        if let Some(installed_db) = read_installed_db(&local_db_dir, name, &version_str) {
            if sync_repo_names.iter().any(|r| r == &installed_db) {
                continue;
            }
        } else if pkg.validation() != PackageValidation::NONE {
            continue;
        }

        if is_ignored(&pacman_conf.ignore_pkg, name) {
            continue;
        }

        let mut hit: Option<(String, String)> = None;
        for db in alpm.syncdbs() {
            if let Ok(sync_pkg) = db.pkg(name) {
                hit = Some((db.name().to_string(), sync_pkg.version().to_string()));
                break;
            }
        }

        if let Some((target_repo, target_version)) = hit {
            if vercmp(target_version.as_str(), version_str.as_str()) == Ordering::Less {
                continue;
            }

            switches.push(RepoSwitch {
                kind: SwitchKind::RepoChange,
                installed_name: name.to_string(),
                installed_repo: PackageSource::Aur.label().to_string(),
                installed_version: version_str.clone(),
                target_name: name.to_string(),
                target_repo,
                target_version,
            });
        }
    }

    for db in alpm.syncdbs() {
        for sync_pkg in db.pkgs() {
            for replaces_dep in sync_pkg.replaces() {
                let replaced_name = replaces_dep.name();

                if replaced_name == sync_pkg.name() {
                    continue;
                }

                let Some(installed_version) = installed_meta.get(replaced_name) else {
                    continue;
                };

                if !dep_satisfies(&replaces_dep, installed_version) {
                    continue;
                }

                if local.pkg(sync_pkg.name()).is_ok() {
                    continue;
                }

                if is_ignored(&pacman_conf.ignore_pkg, replaced_name)
                    || is_ignored(&pacman_conf.ignore_pkg, sync_pkg.name())
                {
                    continue;
                }

                let target_version = sync_pkg.version().to_string();
                if vercmp(target_version.as_str(), installed_version.as_str()) == Ordering::Less {
                    continue;
                }

                let already_listed = switches.iter().any(|s| {
                    s.kind == SwitchKind::Replace
                        && s.installed_name == replaced_name
                        && s.target_name == sync_pkg.name()
                });
                if already_listed {
                    continue;
                }

                switches.push(RepoSwitch {
                    kind: SwitchKind::Replace,
                    installed_name: replaced_name.to_string(),
                    installed_repo: "local".to_string(),
                    installed_version: installed_version.clone(),
                    target_name: sync_pkg.name().to_string(),
                    target_repo: db.name().to_string(),
                    target_version,
                });
            }
        }
    }

    return Ok(switches);
}

fn is_ignored(ignore_pkg: &[String], name: &str) -> bool {
    return ignore_pkg.iter().any(|p| p == name);
}

fn dep_satisfies(dep: &alpm::Dep, installed_version: &str) -> bool {
    use alpm::DepModVer;
    let cmp = |v: &alpm::Ver| vercmp(installed_version, v.to_string().as_str());
    return match dep.depmodver() {
        DepModVer::Any => true,
        DepModVer::Eq(v) => cmp(v) == Ordering::Equal,
        DepModVer::Ge(v) => cmp(v) != Ordering::Less,
        DepModVer::Le(v) => cmp(v) != Ordering::Greater,
        DepModVer::Gt(v) => cmp(v) == Ordering::Greater,
        DepModVer::Lt(v) => cmp(v) == Ordering::Less,
    };
}

fn read_installed_db(local_db_dir: &PathBuf, name: &str, version: &str) -> Option<String> {
    let desc_path = local_db_dir
        .join(format!("{}-{}", name, version))
        .join("desc");
    let content = fs::read_to_string(&desc_path).ok()?;
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        if line.trim() == "%INSTALLED_DB%" {
            if let Some(value) = lines.next() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return None;
                }
                return Some(trimmed.to_string());
            }
            return None;
        }
    }
    return None;
}
