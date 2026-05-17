use std::process::Command;

pub fn is_snapper_installed() -> bool {
    return Command::new("which")
        .arg("snapper")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

pub fn is_snap_pac_installed() -> bool {
    return Command::new("pacman")
        .args(&["-Q", "snap-pac"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

pub fn build_snapper_snapshot_command() -> String {
    return "sudo snapper -c root create --type single --description \"arch-update-manager\""
        .to_string();
}
