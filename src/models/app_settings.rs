use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::models::snapshot_retention_period::SnapshotRetentionPeriod;

impl Display for SnapshotRetentionPeriod {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotRetentionPeriod::Forever => write!(f, "Forever"),
            SnapshotRetentionPeriod::Day => write!(f, "1 Day"),
            SnapshotRetentionPeriod::Week => write!(f, "1 Week"),
            SnapshotRetentionPeriod::Month => write!(f, "1 Month"),
            SnapshotRetentionPeriod::Year => write!(f, "1 Year"),
        }
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
    #[serde(default = "default_detect_repo_switches")]
    pub detect_repo_switches: bool,
    #[serde(default = "default_enable_favorites")]
    pub enable_favorites: bool,
    #[serde(default)]
    pub show_favorites_column: bool,
    #[serde(default)]
    pub favorite_packages: Vec<String>,
}

fn default_remember_unselected() -> bool {
    true
}

fn default_enable_favorites() -> bool {
    true
}

fn default_detect_repo_switches() -> bool {
    true
}

fn default_snapshot_retention_count() -> u32 {
    1
}
