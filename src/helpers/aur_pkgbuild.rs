use crate::{
    helpers::aur::url_encode, helpers::elevated::get_original_user, helpers::network::http_get,
    models::pkgbuild_review::PkgbuildReview,
};
use anyhow::{Context, Result};
use std::path::PathBuf;

const PKGBUILD_FETCH_TIMEOUT_SECS: u32 = 10;

pub fn prepare_pkgbuild_review(package: &str) -> Result<PkgbuildReview> {
    let new_content = fetch_remote_pkgbuild(package)?;

    let (old_content, old_label) = match find_cached_pkgbuild(package) {
        Some((path, content)) => (Some(content), format!("Installed ({})", path.display())),
        None => (None, "No cached PKGBUILD found".to_string()),
    };

    return Ok(PkgbuildReview {
        package: package.to_string(),
        old_content,
        old_label,
        new_content,
        new_label: "AUR (latest)".to_string(),
    });
}

fn fetch_remote_pkgbuild(package: &str) -> Result<String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        url_encode(package)
    );
    return http_get(&url, PKGBUILD_FETCH_TIMEOUT_SECS)
        .with_context(|| format!("Could not download the PKGBUILD for {}", package));
}

fn find_cached_pkgbuild(package: &str) -> Option<(PathBuf, String)> {
    let cache = PathBuf::from(user_home()?).join(".cache");

    let candidates = [
        cache.join("yay").join(package).join("PKGBUILD"),
        cache
            .join("paru")
            .join("clone")
            .join(package)
            .join("PKGBUILD"),
        cache.join("paru").join(package).join("PKGBUILD"),
        cache.join("trizen").join(package).join("PKGBUILD"),
        cache
            .join("pikaur")
            .join("aur_repos")
            .join(package)
            .join("PKGBUILD"),
    ];

    for path in candidates {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some((path, content));
        }
    }

    return None;
}

fn user_home() -> Option<String> {
    if let Some(user) = get_original_user() {
        return Some(format!("/home/{}", user));
    }
    return std::env::var("HOME").ok();
}
