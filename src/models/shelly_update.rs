#[derive(serde::Deserialize)]
pub struct ShellyUpdate {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub current_version: String,
    #[serde(rename = "NewVersion")]
    pub new_version: String,
    #[serde(rename = "DownloadSize", default)]
    pub download_size: i64,
}
