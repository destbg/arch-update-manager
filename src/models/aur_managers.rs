#[derive(Debug, Clone, PartialEq)]
pub enum AurManagers {
    Yay,
    Paru,
    Trizen,
    Pikaur,
    PamacCli,
    Shelly,
}

impl AurManagers {
    pub fn command(&self) -> &'static str {
        return match self {
            AurManagers::Yay => "yay",
            AurManagers::Paru => "paru",
            AurManagers::Trizen => "trizen",
            AurManagers::Pikaur => "pikaur",
            AurManagers::PamacCli => "pamac",
            AurManagers::Shelly => "shelly",
        };
    }

    pub fn from_command(command: &str) -> Option<Self> {
        return match command {
            "yay" => Some(AurManagers::Yay),
            "paru" => Some(AurManagers::Paru),
            "trizen" => Some(AurManagers::Trizen),
            "pikaur" => Some(AurManagers::Pikaur),
            "pamac" => Some(AurManagers::PamacCli),
            "shelly" => Some(AurManagers::Shelly),
            _ => None,
        };
    }

    pub fn update_check_args(&self) -> Vec<&'static str> {
        return match self {
            AurManagers::Yay => vec!["-Qua"],
            AurManagers::Paru => vec!["-Qua"],
            AurManagers::Trizen => vec!["-Qua"],
            AurManagers::Pikaur => vec!["-Qua"],
            AurManagers::PamacCli => vec!["list", "-u", "-a"],
            AurManagers::Shelly => vec!["aur", "list-updates", "--json"],
        };
    }

    pub fn install_args(&self) -> Vec<&'static str> {
        return match self {
            AurManagers::Yay => vec!["-S"],
            AurManagers::Paru => vec!["-S"],
            AurManagers::Trizen => vec!["-S"],
            AurManagers::Pikaur => vec!["-S"],
            AurManagers::PamacCli => vec!["install"],
            AurManagers::Shelly => vec!["aur", "update"],
        };
    }

    pub fn info_args(&self) -> Vec<&'static str> {
        return match self {
            AurManagers::Yay => vec!["-Si"],
            AurManagers::Paru => vec!["-Sai"],
            AurManagers::Trizen => vec!["-Si"],
            AurManagers::Pikaur => vec!["-Si"],
            AurManagers::PamacCli => vec![],
            AurManagers::Shelly => vec![],
        };
    }

    pub fn devel_args(&self) -> Vec<&'static str> {
        return match self {
            AurManagers::Yay => vec!["--devel"],
            AurManagers::Paru => vec!["--devel"],
            AurManagers::Trizen => vec!["--devel"],
            AurManagers::Pikaur => vec!["--devel"],
            AurManagers::PamacCli => vec![],
            AurManagers::Shelly => vec![],
        };
    }

    pub fn supports_devel(&self) -> bool {
        return !self.devel_args().is_empty();
    }
}
