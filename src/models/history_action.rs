#[derive(Clone, Debug)]
pub struct HistoryAction {
    pub action: String,
    pub package: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
}
