#[derive(Debug)]
pub struct SecurityFix {
    pub severity: String,
    pub fixed: String,
    pub issues: Vec<String>,
}
