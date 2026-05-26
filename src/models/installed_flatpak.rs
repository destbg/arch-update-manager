use crate::models::flatpak_installation::FlatpakInstallation;

pub struct InstalledFlatpak {
    pub name: String,
    pub version: String,
    pub installation: FlatpakInstallation,
}
