use serde::{Deserialize, Serialize};

use crate::models::appimage_update_source::AppImageUpdateSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppImageEntry {
    pub path: String,
    pub name: String,
    pub source: AppImageUpdateSource,
}
