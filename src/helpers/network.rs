use anyhow::Result;
use std::process::Command;

pub fn http_get(url: &str, timeout_secs: u32) -> Result<String> {
    let timeout = timeout_secs.to_string();

    if let Ok(output) = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            &timeout,
            "--connect-timeout",
            &timeout,
            url,
        ])
        .output()
    {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
    }

    if let Ok(output) = Command::new("wget")
        .args(["-q", "-T", &timeout, "-t", "1", "-O", "-", url])
        .output()
    {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
    }

    return Err(anyhow::anyhow!(
        "Failed to fetch {} (curl and wget both failed or timed out)",
        url
    ));
}

pub fn is_network_metered() -> bool {
    let Ok(output) = Command::new("nmcli")
        .args(["-t", "-f", "GENERAL.METERED", "device", "show"])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Some((_, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_ascii_lowercase();
        if value == "yes" || value.starts_with("guess (yes)") || value.starts_with("guess-yes") {
            return true;
        }
    }
    return false;
}
