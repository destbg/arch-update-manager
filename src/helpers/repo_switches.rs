use alpm::{Alpm, PackageValidation, SigLevel, vercmp};
use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::constants::AUR_NAME;
use crate::models::repo_switch::{RepoSwitch, SwitchKind};

pub fn detect_repo_switches() -> Result<Vec<RepoSwitch>> {
    let pacman_conf = pacmanconf::Config::new().context("failed to read pacman.conf")?;

    let alpm = Alpm::new(pacman_conf.root_dir.as_str(), pacman_conf.db_path.as_str())
        .context("failed to initialize alpm handle")?;

    for repo in &pacman_conf.repos {
        alpm.register_syncdb(repo.name.as_str(), SigLevel::NONE)
            .with_context(|| format!("failed to register syncdb {}", repo.name))?;
    }

    let local = alpm.localdb();
    let mut switches: Vec<RepoSwitch> = Vec::new();

    let mut installed_meta: HashMap<String, String> = HashMap::new();
    for pkg in local.pkgs() {
        installed_meta.insert(pkg.name().to_string(), pkg.version().to_string());
    }

    for pkg in local.pkgs() {
        let is_locally_built = pkg.validation() == PackageValidation::NONE;
        if !is_locally_built {
            continue;
        }

        let name = pkg.name();

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
            let installed_version = pkg.version().to_string();
            if vercmp(target_version.as_str(), installed_version.as_str()) == Ordering::Less {
                continue;
            }

            switches.push(RepoSwitch {
                kind: SwitchKind::RepoChange,
                installed_name: name.to_string(),
                installed_repo: AUR_NAME.to_string(),
                installed_version,
                target_name: name.to_string(),
                target_repo,
                target_version,
                selected: false,
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
                    selected: false,
                });
            }
        }
    }

    return Ok(switches);
}

fn is_ignored(ignore_pkg: &[String], name: &str) -> bool {
    return ignore_pkg.iter().any(|p| p == name);
}
