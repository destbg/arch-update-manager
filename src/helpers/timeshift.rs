use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use regex::Regex;
use std::process::Command;

use crate::models::{
    app_settings::AppSettings, snapshot_retention_period::SnapshotRetentionPeriod,
};

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

pub fn cleanup_timeshift_snapshots(
    comment: &str,
    settings: &AppSettings,
    keep_snapshot: &str,
) -> Result<()> {
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

    if same_comment.is_empty() {
        return Ok(());
    }

    let now = Local::now();
    let cutoff_time = match settings.snapshot_retention_period {
        SnapshotRetentionPeriod::Forever => None,
        SnapshotRetentionPeriod::Day => Some(now - Duration::days(1)),
        SnapshotRetentionPeriod::Week => Some(now - Duration::weeks(1)),
        SnapshotRetentionPeriod::Month => Some(now - Duration::days(30)),
        SnapshotRetentionPeriod::Year => Some(now - Duration::days(365)),
    };

    if let Some(cutoff) = cutoff_time {
        same_comment.retain(|(name, _)| {
            if let Ok(snapshot_time) = parse_snapshot_timestamp(name) {
                snapshot_time >= cutoff
            } else {
                true // Keep snapshots we can't parse
            }
        });
    }

    if same_comment.len() > settings.snapshot_retention_count as usize {
        let keep_count = settings.snapshot_retention_count as usize;
        let to_delete = same_comment.len() - keep_count;
        let snapshots_to_delete: Vec<_> = same_comment.drain(0..to_delete).collect();

        for (name, _) in snapshots_to_delete {
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
    }

    return Ok(());
}

fn parse_snapshot_timestamp(snapshot_name: &str) -> Result<DateTime<Local>> {
    // Parse snapshot name format: YYYY-MM-DD_HH-MM-SS
    let re =
        Regex::new(r"([0-9]{4})-([0-9]{2})-([0-9]{2})_([0-9]{2})-([0-9]{2})-([0-9]{2})").unwrap();

    if let Some(caps) = re.captures(snapshot_name) {
        let year: i32 = caps[1].parse()?;
        let month: u32 = caps[2].parse()?;
        let day: u32 = caps[3].parse()?;
        let hour: u32 = caps[4].parse()?;
        let minute: u32 = caps[5].parse()?;
        let second: u32 = caps[6].parse()?;

        let naive_date = NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| anyhow!("Invalid date in snapshot name"))?;
        let naive_time = NaiveTime::from_hms_opt(hour, minute, second)
            .ok_or_else(|| anyhow!("Invalid time in snapshot name"))?;
        let naive_datetime = NaiveDateTime::new(naive_date, naive_time);

        let local_datetime = Local
            .from_local_datetime(&naive_datetime)
            .single()
            .ok_or_else(|| anyhow!("Could not convert to local time"))?;

        return Ok(local_datetime);
    }

    return Err(anyhow!(
        "Could not parse snapshot timestamp from name: {}",
        snapshot_name
    ));
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
