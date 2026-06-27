#[derive(Clone, Debug)]
pub struct AurScanFinding {
    pub id: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub snippet: Option<String>,
}

impl AurScanFinding {
    pub fn severity_rank(&self) -> u8 {
        return match self.severity.to_ascii_lowercase().as_str() {
            "critical" => 5,
            "high" => 4,
            "medium" => 3,
            "low" => 2,
            "info" => 1,
            _ => 0,
        };
    }
}
