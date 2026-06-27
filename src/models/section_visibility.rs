#[derive(Default)]
pub struct SectionVisibility {
    pub orphans: bool,
    pub cache: bool,
    pub pacnew: bool,
    pub services: bool,
    pub flatpak_unused: bool,
    pub resolutions: bool,
}
