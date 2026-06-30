pub const TIMESHIFT_COMMENT: &str = "arch-update-manager";
pub const APP_ID: &str = "com.destbg.arch-update-manager";

pub const OWN_PACKAGES: [&str; 3] = [
    "arch-update-manager",
    "arch-update-manager-bin",
    "arch-update-manager-git",
];

pub fn is_own_package(name: &str) -> bool {
    return OWN_PACKAGES.contains(&name);
}
