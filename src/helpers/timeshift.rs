use anyhow::{Context, Result, anyhow};
use regex::Regex;
use std::process::Command;

pub fn create_timeshift_snapshot(comment: &str) -> Result<String> {
    let status = Command::new("timeshift")
        .args(["--create", "--tags", "O", "--comments", comment, "--yes"])
        .status()?;
    if !status.success() {
        return Err(anyhow!("timeshift snapshot create failed"));
    }

    let snaps = list_timeshift_snapshots_with_comments()?;
    let mut candidates: Vec<_> = snaps
        .into_iter()
        .filter(|(_, c)| c.as_deref().unwrap_or("") == comment)
        .collect();
    if candidates.is_empty() {
        return Err(anyhow!("created snapshot not found in list"));
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    return Ok(candidates.last().unwrap().0.clone());
}

pub fn delete_old_timeshift_snapshot(comment: &str, keep_snapshot: &str) -> Result<()> {
    let mut snaps = list_timeshift_snapshots_with_comments()?;
    for (n, c) in snaps.iter_mut() {
        *n = n.trim().to_string();
        if let Some(s) = c.as_mut() {
            *s = s.trim().to_string();
        }
    }
    snaps.sort_by(|a, b| a.0.cmp(&b.0));

    let mut same_comment: Vec<(String, Option<String>)> = snaps
        .into_iter()
        .filter(|(_, c)| c.as_deref() == Some(comment))
        .collect();

    if same_comment.len() <= 1 {
        println!("No previous snapshot with comment \"{comment}\" to delete.");
        return Ok(());
    }

    same_comment.truncate(same_comment.len() - 1);
    for (name, _) in same_comment {
        if name == keep_snapshot {
            continue;
        }
        let status = Command::new("timeshift")
            .args(["--delete", "--snapshot", &name, "--yes"])
            .status()?;
        if status.success() {
            println!("Deleted snapshot {name}");
        } else {
            return Err(anyhow!("failed to delete snapshot {name}"));
        }
    }
    return Ok(());
}

fn list_timeshift_snapshots_with_comments() -> Result<Vec<(String, Option<String>)>> {
    let out = Command::new("timeshift")
        .args(["--list"])
        .output()
        .context("timeshift --list")?;
    if !out.status.success() {
        return Err(anyhow!("timeshift --list failed"));
    }
    let s = String::from_utf8_lossy(&out.stdout);

    let re_table = Regex::new(
        r"(?m)^\s*\d+\s+(?:>\s+)?([0-9]{4}-[0-9]{2}-[0-9]{2}_[0-9]{2}-[0-9]{2}-[0-9]{2})\s+\S+(?:\s+(.*\S))?\s*$",
    )
    .unwrap();

    let mut result: Vec<(String, Option<String>)> = re_table
        .captures_iter(&s)
        .map(|cap| {
            let name = cap[1].to_string();
            let comment = cap.get(2).map(|m| m.as_str().trim().to_string());
            (name, comment)
        })
        .collect();

    if !result.is_empty() {
        return Ok(result);
    }

    let outv = Command::new("timeshift")
        .args(["--list", "--verbose"])
        .output()
        .context("timeshift --list --verbose")?;
    if !outv.status.success() {
        return Err(anyhow!("timeshift --list --verbose failed"));
    }
    let sv = String::from_utf8_lossy(&outv.stdout);

    let re_snap = Regex::new(
        r"(?m)^\s*Snapshot\s*:\s*([0-9]{4}-[0-9]{2}-[0-9]{2}_[0-9]{2}-[0-9]{2}-[0-9]{2})\s*$",
    )
    .unwrap();
    let re_comm = Regex::new(r"(?m)^\s*Comments\s*:\s*(.*)\s*$").unwrap();

    result.clear();
    let mut cur_name: Option<String> = None;
    let mut cur_comment: Option<String> = None;

    for line in sv.lines() {
        if let Some(cap) = re_snap.captures(line) {
            if let Some(name) = cur_name.take() {
                result.push((name, cur_comment.take()));
            }
            cur_name = Some(cap[1].to_string());
            cur_comment = None;
        } else if let Some(cap) = re_comm.captures(line) {
            cur_comment = Some(cap[1].trim().to_string());
        }
    }
    if let Some(name) = cur_name {
        result.push((name, cur_comment));
    }

    return Ok(result);
}
