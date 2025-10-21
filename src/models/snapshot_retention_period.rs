use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SnapshotRetentionPeriod {
    Forever,
    Day,
    Week,
    Month,
    Year,
}

impl Default for SnapshotRetentionPeriod {
    fn default() -> Self {
        SnapshotRetentionPeriod::Forever
    }
}
