use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::models::check_schedule::CheckSchedule;
use crate::models::snapshot_retention_period::SnapshotRetentionPeriod;

impl Display for SnapshotRetentionPeriod {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            SnapshotRetentionPeriod::Forever => write!(f, "Forever"),
            SnapshotRetentionPeriod::Day => write!(f, "1 Day"),
            SnapshotRetentionPeriod::Week => write!(f, "1 Week"),
            SnapshotRetentionPeriod::Month => write!(f, "1 Month"),
            SnapshotRetentionPeriod::Year => write!(f, "1 Year"),
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub enable_aur_support: bool,
    pub preferred_aur_helper: Option<String>,
    pub create_timeshift_snapshot: bool,
    #[serde(default = "default_snapshot_retention_count")]
    pub snapshot_retention_count: u32,
    #[serde(default)]
    pub snapshot_retention_period: SnapshotRetentionPeriod,
    #[serde(default)]
    pub separate_repository_groups: bool,
    #[serde(default)]
    pub separate_repositories: Vec<String>,
    #[serde(default = "default_remember_unselected")]
    pub remember_unselected_packages: bool,
    #[serde(default = "default_enable_favorites")]
    pub enable_favorites: bool,
    #[serde(default)]
    pub show_favorites_column: bool,
    #[serde(default)]
    pub favorite_packages: Vec<String>,
    #[serde(default)]
    pub favorites_exclusion_mode: bool,
    #[serde(default = "default_enable_flatpak_support")]
    pub enable_flatpak_support: bool,
    #[serde(default = "default_enable_appimage_support")]
    pub enable_appimage_support: bool,
    #[serde(default)]
    pub enable_devel_aur: bool,
    #[serde(default = "default_keep_old_packages")]
    pub keep_old_packages: u32,
    #[serde(default)]
    pub keep_uninstalled_packages: u32,
    #[serde(default)]
    pub auto_clean_cache: bool,
    #[serde(default = "default_run_post_update_checks")]
    pub run_post_update_checks: bool,
    #[serde(default)]
    pub create_snapper_snapshot: bool,
    #[serde(default)]
    pub enable_system_tray: bool,
    #[serde(default)]
    pub check_schedule: CheckSchedule,
    #[serde(default)]
    pub tray_always_visible: bool,
    #[serde(default)]
    pub tray_only_favorites: bool,
    #[serde(default)]
    pub tray_menu_only_favorites: bool,
    #[serde(default)]
    pub skip_check_on_metered: bool,
    #[serde(default)]
    pub skip_check_on_battery: bool,
    #[serde(default)]
    pub show_update_notifications: bool,
    #[serde(default = "default_show_package_descriptions")]
    pub show_package_descriptions: bool,
    #[serde(default = "default_show_updated_date")]
    pub show_updated_date: bool,
    #[serde(default)]
    pub min_update_age_days: u32,
    #[serde(default)]
    pub min_update_age_aur_only: bool,
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,
    #[serde(default = "default_check_arch_news")]
    pub check_arch_news: bool,
    #[serde(default = "default_enable_mirror_refresh")]
    pub enable_mirror_refresh: bool,
}

impl AppSettings {
    pub fn is_favorite(&self, name: &str) -> bool {
        let in_list = self.favorite_packages.iter().any(|p| p == name);
        if self.favorites_exclusion_mode {
            return !in_list;
        }
        return in_list;
    }

    pub fn set_favorite(&mut self, name: &str, want_favorite: bool) {
        let should_be_in_list = if self.favorites_exclusion_mode {
            !want_favorite
        } else {
            want_favorite
        };
        let in_list = self.favorite_packages.iter().any(|p| p == name);
        if should_be_in_list && !in_list {
            self.favorite_packages.push(name.to_string());
        } else if !should_be_in_list && in_list {
            self.favorite_packages.retain(|p| p != name);
        }
    }
}

fn default_remember_unselected() -> bool {
    return true;
}

fn default_enable_favorites() -> bool {
    return true;
}

fn default_snapshot_retention_count() -> u32 {
    return 1;
}

fn default_enable_flatpak_support() -> bool {
    return std::process::Command::new("which")
        .arg("flatpak")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

fn default_enable_appimage_support() -> bool {
    return true;
}

fn default_keep_old_packages() -> u32 {
    return 3;
}

fn default_run_post_update_checks() -> bool {
    return true;
}

fn default_show_package_descriptions() -> bool {
    return true;
}

fn default_show_updated_date() -> bool {
    return true;
}

fn default_log_retention_days() -> u32 {
    return 7;
}

fn default_check_arch_news() -> bool {
    return true;
}

fn default_enable_mirror_refresh() -> bool {
    return true;
}
