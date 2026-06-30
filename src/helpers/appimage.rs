use anyhow::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::helpers::appimage_config::{load_appimage_entries, source_for_path};
use crate::helpers::elevated::get_original_user;
use crate::helpers::network::http_get;
use crate::models::appimage_update_source::AppImageUpdateSource;
use crate::models::discovered_appimage::DiscoveredAppImage;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;
use crate::models::sha1::Sha1;

const GITHUB_API_TIMEOUT_SECS: u32 = 8;
const ZSYNC_FETCH_TIMEOUT_SECS: u32 = 15;
const HASH_BUFFER_SIZE: usize = 64 * 1024;

pub fn appimage_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join("Applications"));
    }
    return dirs;
}

pub fn discover_appimages() -> Vec<DiscoveredAppImage> {
    let mut found = Vec::new();
    for dir in appimage_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !is_appimage_file(&path) {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("AppImage")
                .to_string();
            found.push(DiscoveredAppImage {
                path: path.to_string_lossy().into_owned(),
                name,
            });
        }
    }
    found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    return found;
}

pub fn managed_appimages() -> Vec<DiscoveredAppImage> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for app in discover_appimages() {
        if seen.insert(app.path.clone()) {
            result.push(app);
        }
    }
    for entry in load_appimage_entries() {
        if !seen.insert(entry.path.clone()) {
            continue;
        }
        if Path::new(&entry.path).is_file() {
            result.push(DiscoveredAppImage {
                path: entry.path,
                name: entry.name,
            });
        }
    }
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    return result;
}

pub fn resolve_source(path: &str, name: &str) -> AppImageUpdateSource {
    if let Some(source) = source_for_path(path) {
        return source;
    }
    let _ = name;
    return embedded_source(path);
}

pub fn embedded_source(path: &str) -> AppImageUpdateSource {
    let Some(info) = read_embedded_update_info(path) else {
        return AppImageUpdateSource::None;
    };
    return parse_update_info(&info);
}

pub fn get_appimage_updates() -> Result<Vec<PackageUpdate>> {
    let mut updates = Vec::new();
    for app in managed_appimages() {
        let source = resolve_source(&app.path, &app.name);
        if matches!(source, AppImageUpdateSource::None) {
            continue;
        }

        let Some((zsync_url, new_version, source_date)) = resolve_zsync(&source) else {
            continue;
        };
        let Some((remote_sha1, target_length, zsync_mtime, download_url)) =
            fetch_zsync_meta(&zsync_url)
        else {
            continue;
        };
        let Some(local_sha1) = file_sha1(&app.path) else {
            continue;
        };
        if local_sha1.eq_ignore_ascii_case(&remote_sha1) {
            continue;
        }

        let size = target_length.max(0);

        updates.push(PackageUpdate {
            source: PackageSource::AppImage,
            repository: PackageSource::AppImage.label().to_string(),
            selected: true,
            name: app.name.clone(),
            description: source_description(&source, &app.path),
            current_version: "installed".to_string(),
            new_version,
            size,
            build_date: source_date.or(zsync_mtime),
            url: Some(download_url),
            appimage_path: Some(app.path.clone()),
            ..Default::default()
        });
    }
    return Ok(updates);
}

pub fn build_appimage_update_commands(packages: &[&PackageUpdate]) -> Vec<String> {
    let mut commands = Vec::new();
    for pkg in packages {
        let Some(path) = pkg.appimage_path.as_ref() else {
            continue;
        };
        let Some(download_url) = pkg
            .url
            .clone()
            .or_else(|| resolve_download_url_for(path, &pkg.name))
        else {
            continue;
        };
        if let Some(command) = build_one_command(&pkg.name, path, &download_url) {
            commands.push(command);
        }
    }
    return commands;
}

fn resolve_download_url_for(path: &str, name: &str) -> Option<String> {
    let source = resolve_source(path, name);
    let (zsync_url, _, _) = resolve_zsync(&source)?;
    let (_, _, _, download_url) = fetch_zsync_meta(&zsync_url)?;
    return Some(download_url);
}

fn build_one_command(name: &str, path: &str, download_url: &str) -> Option<String> {
    let quoted_name = shlex::try_quote(name).ok()?.into_owned();
    let quoted_path = shlex::try_quote(path).ok()?.into_owned();
    let quoted_url = shlex::try_quote(download_url).ok()?.into_owned();
    let prefix = appimage_command_prefix();
    return Some(format!(
        "echo Downloading AppImage update: {name} && {prefix} curl -fSL --progress-bar -o {path}.new -- {url} && chmod +x {path}.new && mv -f -- {path}.new {path}",
        name = quoted_name,
        prefix = prefix,
        path = quoted_path,
        url = quoted_url,
    ));
}

fn appimage_command_prefix() -> String {
    if let Some(user) = get_original_user() {
        if let Ok(quoted) = shlex::try_quote(&user) {
            return format!("sudo -u {}", quoted);
        }
    }
    return String::new();
}

fn resolve_zsync(source: &AppImageUpdateSource) -> Option<(String, String, Option<i64>)> {
    return match source {
        AppImageUpdateSource::None => None,
        AppImageUpdateSource::Zsync { url } => Some((url.clone(), zsync_label(url), None)),
        AppImageUpdateSource::GitHub {
            owner,
            repo,
            prerelease,
        } => github_zsync_asset(owner, repo, *prerelease),
    };
}

fn source_description(source: &AppImageUpdateSource, path: &str) -> String {
    return match source {
        AppImageUpdateSource::GitHub { owner, repo, .. } => {
            format!("AppImage, updates from GitHub {}/{}", owner, repo)
        }
        AppImageUpdateSource::Zsync { url } => {
            format!("AppImage, updates from {}", url_host(url))
        }
        AppImageUpdateSource::None => format!("AppImage: {}", path),
    };
}

fn url_host(url: &str) -> String {
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    return without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .to_string();
}

fn parse_http_date(value: &str) -> Option<i64> {
    return chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|dt| dt.timestamp());
}

fn parse_iso_date(value: &str) -> Option<i64> {
    return chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp());
}

fn zsync_label(url: &str) -> String {
    return url
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .trim_end_matches(".zsync")
        .to_string();
}

fn github_zsync_asset(
    owner: &str,
    repo: &str,
    prerelease: bool,
) -> Option<(String, String, Option<i64>)> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=15",
        owner, repo
    );
    let body = http_get(&url, GITHUB_API_TIMEOUT_SECS).ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let releases = json.as_array()?;

    for release in releases {
        if release
            .get("draft")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let is_prerelease = release
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_prerelease && !prerelease {
            continue;
        }

        let tag = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let published = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .and_then(parse_iso_date);
        let Some(assets) = release.get("assets").and_then(|v| v.as_array()) else {
            continue;
        };

        let mut fallback: Option<String> = None;
        for asset in assets {
            let asset_name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if !asset_name.to_lowercase().ends_with(".zsync") {
                continue;
            }
            let download = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if download.is_empty() {
                continue;
            }
            if asset_name.contains("x86_64") || asset_name.contains("amd64") {
                return Some((download, tag, published));
            }
            if fallback.is_none() {
                fallback = Some(download);
            }
        }
        if let Some(download) = fallback {
            return Some((download, tag, published));
        }
    }
    return None;
}

fn fetch_zsync_meta(zsync_url: &str) -> Option<(String, i64, Option<i64>, String)> {
    let body = http_get(zsync_url, ZSYNC_FETCH_TIMEOUT_SECS).ok()?;
    let mut sha1 = None;
    let mut length = 0i64;
    let mut mtime = None;
    let mut url_header = None;
    for line in body.lines() {
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("SHA-1:") {
            sha1 = Some(rest.trim().to_lowercase());
        } else if let Some(rest) = line.strip_prefix("Length:") {
            length = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("MTime:") {
            mtime = parse_http_date(rest.trim());
        } else if let Some(rest) = line.strip_prefix("URL:") {
            url_header = Some(rest.trim().to_string());
        }
    }
    let download_url = resolve_download_url(zsync_url, url_header.as_deref());
    return Some((sha1?, length, mtime, download_url));
}

fn resolve_download_url(zsync_url: &str, header: Option<&str>) -> String {
    let Some(header) = header.filter(|value| !value.is_empty()) else {
        return zsync_url.trim_end_matches(".zsync").to_string();
    };
    if header.starts_with("http://") || header.starts_with("https://") {
        return header.to_string();
    }
    if let Some(slash) = zsync_url.rfind('/') {
        return format!("{}{}", &zsync_url[..slash + 1], header);
    }
    return header.to_string();
}

fn is_appimage_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("AppImage") {
            return true;
        }
    }
    return has_appimage_magic(path);
}

fn has_appimage_magic(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 11];
    if file.read_exact(&mut buf).is_err() {
        return false;
    }
    let is_elf = &buf[0..4] == b"\x7fELF";
    let is_appimage = buf[8] == 0x41 && buf[9] == 0x49 && (buf[10] == 0x01 || buf[10] == 0x02);
    return is_elf && is_appimage;
}

fn read_embedded_update_info(path: &str) -> Option<String> {
    let mut file = File::open(path).ok()?;

    let mut header = [0u8; 64];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"\x7fELF" {
        return None;
    }
    if header[4] != 2 || header[5] != 1 {
        return None;
    }

    let sh_offset = read_u64_le(&header, 0x28);
    let sh_entry_size = read_u16_le(&header, 0x3a) as u64;
    let sh_count = read_u16_le(&header, 0x3c) as u64;
    let sh_str_index = read_u16_le(&header, 0x3e) as u64;
    if sh_entry_size < 64 || sh_count == 0 || sh_str_index >= sh_count {
        return None;
    }

    let str_table = read_section_header(&mut file, sh_offset, sh_entry_size, sh_str_index)?;
    let str_table_bytes = read_section_bytes(&mut file, str_table.0, str_table.1)?;

    for index in 0..sh_count {
        let (name_offset, section_offset, section_size) =
            read_full_section_header(&mut file, sh_offset, sh_entry_size, index)?;
        let name = section_name(&str_table_bytes, name_offset as usize);
        if name == ".upd_info" {
            let bytes = read_section_bytes(&mut file, section_offset, section_size)?;
            let text = String::from_utf8_lossy(&bytes);
            let trimmed = text.trim_matches(char::from(0)).trim();
            if trimmed.is_empty() {
                return None;
            }
            return Some(trimmed.to_string());
        }
    }
    return None;
}

fn read_section_header(
    file: &mut File,
    table_offset: u64,
    entry_size: u64,
    index: u64,
) -> Option<(u64, u64)> {
    let (_, offset, size) = read_full_section_header(file, table_offset, entry_size, index)?;
    return Some((offset, size));
}

fn read_full_section_header(
    file: &mut File,
    table_offset: u64,
    entry_size: u64,
    index: u64,
) -> Option<(u32, u64, u64)> {
    let entry_offset = table_offset.checked_add(entry_size.checked_mul(index)?)?;
    file.seek(SeekFrom::Start(entry_offset)).ok()?;
    let mut entry = [0u8; 64];
    file.read_exact(&mut entry).ok()?;
    let name_offset = read_u32_le(&entry, 0x00);
    let section_offset = read_u64_le(&entry, 0x18);
    let section_size = read_u64_le(&entry, 0x20);
    return Some((name_offset, section_offset, section_size));
}

fn read_section_bytes(file: &mut File, offset: u64, size: u64) -> Option<Vec<u8>> {
    if size == 0 || size > 64 * 1024 {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = vec![0u8; size as usize];
    file.read_exact(&mut bytes).ok()?;
    return Some(bytes);
}

fn section_name(str_table: &[u8], offset: usize) -> String {
    if offset >= str_table.len() {
        return String::new();
    }
    let end = str_table[offset..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| offset + p)
        .unwrap_or(str_table.len());
    return String::from_utf8_lossy(&str_table[offset..end]).into_owned();
}

fn parse_update_info(info: &str) -> AppImageUpdateSource {
    let parts: Vec<&str> = info.split('|').collect();
    return match parts.as_slice() {
        ["zsync", url] => AppImageUpdateSource::Zsync {
            url: (*url).to_string(),
        },
        ["gh-releases-zsync", owner, repo, _tag, _glob] => AppImageUpdateSource::GitHub {
            owner: (*owner).to_string(),
            repo: (*repo).to_string(),
            prerelease: false,
        },
        _ => AppImageUpdateSource::None,
    };
}

fn file_sha1(path: &str) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; HASH_BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    return Some(hasher.finalize_hex());
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    return u16::from_le_bytes([data[offset], data[offset + 1]]);
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    return u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
}

fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[offset..offset + 8]);
    return u64::from_le_bytes(bytes);
}
