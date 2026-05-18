#[derive(serde::Deserialize)]
pub struct ShellyUpdate {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "CurrentVersion")]
    pub current_version: String,
    #[serde(rename = "NewVersion")]
    pub new_version: String,
    #[serde(rename = "SizeDifference", default)]
    pub size_difference: i64,
}
