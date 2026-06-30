use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrayState {
    pub last_check: Option<DateTime<Utc>>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub aur: Vec<String>,
    #[serde(default)]
    pub flatpak: Vec<String>,
    #[serde(default)]
    pub appimage: Vec<String>,
}

impl TrayState {
    pub fn total(&self) -> usize {
        return self.packages.len() + self.aur.len() + self.flatpak.len() + self.appimage.len();
    }
}
