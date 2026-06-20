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
    pub first_submitted: Option<i64>,
    pub out_of_date: Option<i64>,
    pub orphaned: bool,
    pub maintainer: Option<String>,
    pub previous_maintainer: Option<String>,
    pub num_votes: Option<i64>,
    pub popularity: Option<f64>,
    pub security_severity: Option<String>,
    pub security_issues: Vec<String>,
    pub new_permissions: Vec<String>,
    pub flatpak_installation: Option<FlatpakInstallation>,
}

impl PackageUpdate {
    pub fn maintainer_changed(&self) -> bool {
        return self.previous_maintainer.is_some()
            && self.previous_maintainer.as_deref() != self.maintainer.as_deref();
    }
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
            first_submitted: None,
            out_of_date: None,
            orphaned: false,
            maintainer: None,
            previous_maintainer: None,
            num_votes: None,
            popularity: None,
            security_severity: None,
            security_issues: Vec::new(),
            new_permissions: Vec::new(),
            flatpak_installation: None,
        }
    }
}
