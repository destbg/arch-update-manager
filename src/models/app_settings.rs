use serde::{Deserialize, Serialize};

use crate::models::snapshot_retention_period::SnapshotRetentionPeriod;

impl std::fmt::Display for SnapshotRetentionPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
}

fn default_snapshot_retention_count() -> u32 {
    1
}
