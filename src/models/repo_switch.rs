#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchKind {
    RepoChange,
    Replace,
}

#[derive(Debug, Clone)]
pub struct RepoSwitch {
    pub kind: SwitchKind,
    pub installed_name: String,
    pub installed_repo: String,
    pub installed_version: String,
    pub target_name: String,
    pub target_repo: String,
    pub target_version: String,
}
