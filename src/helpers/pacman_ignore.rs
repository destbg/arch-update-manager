use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const PACMAN_CONF: &str = "/etc/pacman.conf";
const MARKER_BEGIN: &str = "# arch-update-manager managed begin - do not edit";
const MARKER_END: &str = "# arch-update-manager managed end";

pub fn list_managed_ignores() -> Vec<String> {
    let Ok(content) = fs::read_to_string(PACMAN_CONF) else {
        return Vec::new();
    };
    return extract_managed_ignores(&content);
}

pub fn is_in_managed_ignore_pkg(package: &str) -> bool {
    return list_managed_ignores().iter().any(|p| p == package);
}

pub fn add_to_ignore_pkg(package: &str) -> Result<()> {
    let content = fs::read_to_string(PACMAN_CONF).context("Failed to read pacman.conf")?;
    let mut entries = extract_managed_ignores(&content);
    if entries.iter().any(|p| p == package) {
        return Ok(());
    }
    entries.push(package.to_string());
    entries.sort();
    entries.dedup();
    let new_content = rewrite_with_managed_block(&content, &entries);
    return write_pacman_conf(&new_content);
}

pub fn remove_from_ignore_pkg(package: &str) -> Result<()> {
    let content = fs::read_to_string(PACMAN_CONF).context("Failed to read pacman.conf")?;
    let mut entries = extract_managed_ignores(&content);
    let original_len = entries.len();
    entries.retain(|p| p != package);
    if entries.len() == original_len {
        return Ok(());
    }
    let new_content = rewrite_with_managed_block(&content, &entries);
    return write_pacman_conf(&new_content);
}

fn extract_managed_ignores(content: &str) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut in_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == MARKER_BEGIN {
            in_block = true;
            continue;
        }
        if trimmed == MARKER_END {
            in_block = false;
            continue;
        }
        if !in_block {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("IgnorePkg") {
            let rest = rest.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
            for pkg in rest.split_whitespace() {
                entries.push(pkg.to_string());
            }
        }
    }
    return entries;
}

fn rewrite_with_managed_block(content: &str, entries: &[String]) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let trailing_newline = content.ends_with('\n');

    strip_managed_block(&mut lines);

    if !entries.is_empty() {
        let block = build_managed_block(entries);
        insert_block_at_end_of_options(&mut lines, block);
    }

    let mut joined = lines.join("\n");
    if trailing_newline {
        joined.push('\n');
    }
    return joined;
}

fn strip_managed_block(lines: &mut Vec<String>) {
    let begin_idx = lines.iter().position(|l| l.trim() == MARKER_BEGIN);
    let end_idx = lines.iter().position(|l| l.trim() == MARKER_END);

    if let (Some(begin), Some(end)) = (begin_idx, end_idx) {
        if end >= begin {
            let mut drain_end = end + 1;
            if drain_end < lines.len() && lines[drain_end].trim().is_empty() {
                drain_end += 1;
            }
            let mut drain_start = begin;
            if drain_start > 0 && lines[drain_start - 1].trim().is_empty() {
                drain_start -= 1;
            }
            lines.drain(drain_start..drain_end);
        }
    }
}

fn build_managed_block(entries: &[String]) -> Vec<String> {
    return vec![
        String::new(),
        MARKER_BEGIN.to_string(),
        format!("IgnorePkg = {}", entries.join(" ")),
        MARKER_END.to_string(),
    ];
}

fn insert_block_at_end_of_options(lines: &mut Vec<String>, block: Vec<String>) {
    let options_idx = lines.iter().position(|l| l.trim() == "[options]");

    let insert_at = match options_idx {
        Some(start) => {
            let mut idx = lines.len();
            for (i, line) in lines.iter().enumerate().skip(start + 1) {
                let t = line.trim();
                if t.starts_with('[') && t.ends_with(']') {
                    idx = i;
                    break;
                }
            }
            while idx > 0 && lines[idx - 1].trim().is_empty() {
                idx -= 1;
            }
            idx
        }
        None => lines.len(),
    };

    for (offset, line) in block.into_iter().enumerate() {
        lines.insert(insert_at + offset, line);
    }
}

fn write_pacman_conf(content: &str) -> Result<()> {
    let target = Path::new(PACMAN_CONF);
    let parent = target.parent().unwrap_or_else(|| Path::new("/"));
    let tmp: PathBuf = parent.join(".pacman.conf.arch-update-manager.tmp");

    fs::write(&tmp, content).with_context(|| format!("Failed to write {}", tmp.display()))?;
    fs::rename(&tmp, target).with_context(|| format!("Failed to replace {}", target.display()))?;
    return Ok(());
}
