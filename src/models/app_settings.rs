use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub enable_aur_support: bool,
    pub preferred_aur_helper: Option<String>,
    pub create_timeshift_snapshot: bool,
}
