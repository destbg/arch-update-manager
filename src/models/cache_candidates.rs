#[derive(Debug, Default, Clone)]
pub struct CacheCandidates {
    pub old_count: u32,
    pub uninstalled_count: u32,
    pub disk_space: Option<String>,
    pub old_packages: Vec<String>,
    pub uninstalled_packages: Vec<String>,
}
