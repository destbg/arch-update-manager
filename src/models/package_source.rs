#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSource {
    Official,
    Aur,
    Flatpak,
    AppImage,
}

impl PackageSource {
    pub fn label(&self) -> &'static str {
        return match self {
            PackageSource::Official => "official",
            PackageSource::Aur => "aur",
            PackageSource::Flatpak => "flatpak",
            PackageSource::AppImage => "appimage",
        };
    }
}
