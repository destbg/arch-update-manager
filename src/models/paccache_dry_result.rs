pub struct PaccacheDryResult {
    pub count: u32,
    pub space: Option<(u64, String)>,
    pub packages: Vec<String>,
}
