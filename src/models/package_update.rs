use crate::models::flatpak_installation::FlatpakInstallation;

#[derive(Clone, Debug)]
pub struct PackageUpdate {
    pub repository: String,
    pub selected: bool,
    pub name: String,
    pub description: String,
    pub current_version: String,
    pub new_version: String,
    pub size: i64,
    pub url: Option<String>,
    pub build_date: Option<i64>,
    pub out_of_date: Option<i64>,
    pub orphaned: bool,
    pub security_severity: Option<String>,
    pub security_issues: Vec<String>,
    pub flatpak_installation: Option<FlatpakInstallation>,
}

impl Default for PackageUpdate {
    fn default() -> Self {
        Self {
            repository: String::new(),
            selected: false,
            name: String::new(),
            description: String::new(),
            current_version: String::new(),
            new_version: String::new(),
            size: 0,
            url: None,
            build_date: None,
            out_of_date: None,
            orphaned: false,
            security_severity: None,
            security_issues: Vec::new(),
            flatpak_installation: None,
        }
    }
}
