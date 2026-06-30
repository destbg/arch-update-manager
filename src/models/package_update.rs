use crate::models::aur_scan_finding::AurScanFinding;
use crate::models::flatpak_installation::FlatpakInstallation;
use crate::models::package_source::PackageSource;

#[derive(Clone, Debug)]
pub struct PackageUpdate {
    pub source: PackageSource,
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
    pub extra_dependencies: Vec<String>,
    pub pkgbuild_needs_review: bool,
    pub aur_scan_findings: Vec<AurScanFinding>,
    pub flatpak_installation: Option<FlatpakInstallation>,
    pub appimage_path: Option<String>,
}

impl PackageUpdate {
    pub fn maintainer_changed(&self) -> bool {
        return self.previous_maintainer.is_some()
            && self.previous_maintainer.as_deref() != self.maintainer.as_deref();
    }

    pub fn aur_scan_summary(&self) -> Option<(String, usize)> {
        let worst = self
            .aur_scan_findings
            .iter()
            .max_by_key(|f| f.severity_rank())?;
        return Some((worst.severity.clone(), self.aur_scan_findings.len()));
    }
}

impl Default for PackageUpdate {
    fn default() -> Self {
        return Self {
            source: PackageSource::Official,
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
            extra_dependencies: Vec::new(),
            pkgbuild_needs_review: false,
            aur_scan_findings: Vec::new(),
            flatpak_installation: None,
            appimage_path: None,
        };
    }
}
