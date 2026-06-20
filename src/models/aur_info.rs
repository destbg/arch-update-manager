#[derive(Debug)]
pub struct AurInfo {
    pub description: Option<String>,
    pub url: Option<String>,
    pub last_modified: Option<i64>,
    pub first_submitted: Option<i64>,
    pub out_of_date: Option<i64>,
    pub maintainer: Option<String>,
    pub num_votes: Option<i64>,
    pub popularity: Option<f64>,
}
