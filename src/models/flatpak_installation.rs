#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FlatpakInstallation {
    User,
    System,
}

impl FlatpakInstallation {
    pub fn flag(self) -> &'static str {
        return match self {
            FlatpakInstallation::User => "--user",
            FlatpakInstallation::System => "--system",
        };
    }
}
