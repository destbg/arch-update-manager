use std::process::Command;

use anyhow::{Result, anyhow};

pub fn list_package_files(name: &str) -> Result<Vec<String>> {
    let output = Command::new("pacman").args(["-Ql", name]).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("pacman -Ql failed for {}: {}", name, stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in stdout.lines() {
        if let Some(idx) = line.find(' ') {
            let path = line[idx + 1..].trim().to_string();
            if !path.is_empty() {
                files.push(path);
            }
        }
    }
    return Ok(files);
}
