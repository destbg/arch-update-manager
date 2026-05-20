use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CachedVersion {
    pub version: String,
    pub path: PathBuf,
}
