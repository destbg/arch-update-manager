#[derive(Debug, Clone, serde::Deserialize)]
pub struct ShellyAppImage {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Path", default)]
    pub path: Option<String>,
    #[serde(rename = "UpdateType", default)]
    pub update_type: i64,
    #[serde(rename = "RepoOwner", default)]
    pub repo_owner: Option<String>,
    #[serde(rename = "RepoName", default)]
    pub repo_name: Option<String>,
    #[serde(rename = "UpdateURl", default)]
    pub update_url: String,
    #[serde(rename = "AllowPrerelease", default)]
    pub allow_prerelease: bool,
}
