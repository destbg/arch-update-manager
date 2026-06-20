#[derive(Debug)]
pub struct PackageInfo {
    pub description: String,
    pub repository: String,
    pub url: Option<String>,
    pub depends: Vec<String>,
}
