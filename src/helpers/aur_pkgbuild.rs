use crate::{
    helpers::aur::url_encode, helpers::elevated::get_original_user, helpers::network::http_get,
    models::pkgbuild_review::PkgbuildReview,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const PKGBUILD_FETCH_TIMEOUT_SECS: u32 = 10;

pub fn prepare_pkgbuild_review(package: &str) -> Result<PkgbuildReview> {
    if let Some(dir) = find_clone_dir(package) {
        if let Some((diff, needs_review)) = compute_review(&dir) {
            return Ok(PkgbuildReview {
                package: package.to_string(),
                diff: Some(diff),
                needs_review,
                pkgbuild: None,
            });
        }
    }

    let pkgbuild = fetch_remote_pkgbuild(package)?;
    return Ok(PkgbuildReview {
        package: package.to_string(),
        diff: None,
        needs_review: false,
        pkgbuild: Some(pkgbuild),
    });
}

pub fn pkgbuild_needs_review(package: &str) -> bool {
    if let Some(dir) = find_clone_dir(package) {
        if let Some((_, needs_review)) = compute_review(&dir) {
            return needs_review;
        }
    }
    return false;
}

pub fn find_clone_dir(package: &str) -> Option<PathBuf> {
    let cache = PathBuf::from(user_home()?).join(".cache");

    let candidates = [
        cache.join("paru").join("clone").join(package),
        cache.join("paru").join(package),
        cache.join("yay").join(package),
        cache.join("trizen").join(package),
        cache.join("pikaur").join("aur_repos").join(package),
    ];

    for dir in candidates {
        if dir.join(".git").exists() && dir.join("PKGBUILD").exists() {
            return Some(dir);
        }
    }

    return None;
}

fn compute_review(dir: &Path) -> Option<(String, bool)> {
    let user = get_original_user();
    let dir_str = dir.to_str()?;

    let _ = run_git(user.as_deref(), dir_str, &["fetch", "--quiet"]);

    let output = run_git(
        user.as_deref(),
        dir_str,
        &[
            "diff",
            "--no-color",
            "--unified=100000",
            "HEAD",
            "FETCH_HEAD",
            "--",
            ".",
            ":(exclude).SRCINFO",
            ":(exclude).gitignore",
            ":(exclude).gitattributes",
        ],
    )?;
    if !output.status.success() {
        return None;
    }

    let diff = String::from_utf8_lossy(&output.stdout).into_owned();
    let needs_review = diff_adds_or_removes_lines(&diff);
    return Some((diff, needs_review));
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

fn diff_adds_or_removes_lines(diff: &str) -> bool {
    let mut added = 0i32;
    let mut removed = 0i32;
    let mut in_hunk = false;
    let mut needs = false;

    for line in diff.lines() {
        let is_boundary = line.starts_with("@@")
            || line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("new file")
            || line.starts_with("deleted file")
            || line.starts_with("rename ")
            || line.starts_with("similarity ")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("Binary files");

        if is_boundary {
            if in_hunk && added != removed {
                needs = true;
            }
            added = 0;
            removed = 0;
            in_hunk = line.starts_with("@@");
            continue;
        }

        if in_hunk {
            if line.starts_with('+') {
                added += 1;
            } else if line.starts_with('-') {
                removed += 1;
            }
        }
    }
    if in_hunk && added != removed {
        needs = true;
    }

    return needs;
}

fn fetch_remote_pkgbuild(package: &str) -> Result<String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        url_encode(package)
    );
    return http_get(&url, PKGBUILD_FETCH_TIMEOUT_SECS)
        .with_context(|| format!("Could not download the PKGBUILD for {}", package));
}

fn user_home() -> Option<String> {
    if let Some(user) = get_original_user() {
        return Some(format!("/home/{}", user));
    }
    return std::env::var("HOME").ok();
}
