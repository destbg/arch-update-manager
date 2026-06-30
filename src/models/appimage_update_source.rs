use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppImageUpdateSource {
    None,
    Zsync {
        url: String,
    },
    GitHub {
        owner: String,
        repo: String,
        #[serde(default)]
        prerelease: bool,
    },
}
