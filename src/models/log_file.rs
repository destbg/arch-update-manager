use chrono::{DateTime, Local};
use std::path::PathBuf;

#[derive(Clone)]
pub struct LogFile {
    pub path: PathBuf,
    pub started_at: DateTime<Local>,
}
