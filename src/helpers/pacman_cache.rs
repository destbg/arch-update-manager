use std::fs;
use std::path::PathBuf;

use crate::models::cached_version::CachedVersion;

const CACHE_DIR: &str = "/var/cache/pacman/pkg";

const PACKAGE_SUFFIXES: &[&str] = &[
    ".pkg.tar.zst",
    ".pkg.tar.xz",
    ".pkg.tar.gz",
    ".pkg.tar.bz2",
    ".pkg.tar",
];

pub fn list_cached_versions(name: &str) -> Vec<CachedVersion> {
    let Ok(entries) = fs::read_dir(CACHE_DIR) else {
        return Vec::new();
    };

    let mut out: Vec<CachedVersion> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some((pkg_name, version)) = parse_filename(filename) else {
            continue;
        };
        if pkg_name == name {
            out.push(CachedVersion { version, path });
        }
    }

    out.sort_by(|a, b| b.version.cmp(&a.version));
    return out;
}

pub fn package_path_to_string(path: &PathBuf) -> String {
    return path.to_string_lossy().to_string();
}

fn parse_filename(filename: &str) -> Option<(String, String)> {
    let stripped = strip_suffix(filename)?;
    let mut iter = stripped.rsplitn(4, '-');
    let _arch = iter.next()?;
    let rel = iter.next()?;
    let ver = iter.next()?;
    let name = iter.next()?;
    if name.is_empty() || ver.is_empty() || rel.is_empty() {
        return None;
    }
    return Some((name.to_string(), format!("{}-{}", ver, rel)));
}

fn strip_suffix(filename: &str) -> Option<&str> {
    for suffix in PACKAGE_SUFFIXES {
        if let Some(rest) = filename.strip_suffix(suffix) {
            return Some(rest);
        }
    }
    return None;
}
