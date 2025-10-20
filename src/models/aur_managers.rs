#[derive(Debug, Clone, PartialEq)]
pub enum AurManagers {
    Yay,
    Paru,
    Trizen,
    Pikaur,
    PamacCli,
}

impl AurManagers {
    pub fn command(&self) -> &'static str {
        return match self {
            AurManagers::Yay => "yay",
            AurManagers::Paru => "paru",
            AurManagers::Trizen => "trizen",
            AurManagers::Pikaur => "pikaur",
            AurManagers::PamacCli => "pamac",
        };
    }

    pub fn from_command(command: &str) -> Option<Self> {
        match command {
            "yay" => Some(AurManagers::Yay),
            "paru" => Some(AurManagers::Paru),
            "trizen" => Some(AurManagers::Trizen),
            "pikaur" => Some(AurManagers::Pikaur),
            "pamac" => Some(AurManagers::PamacCli),
            _ => None,
        }
    }

    pub fn update_check_args(&self) -> Vec<&'static str> {
        return match self {
            AurManagers::Yay => vec!["-Qua"],
            AurManagers::Paru => vec!["-Qua"],
            AurManagers::Trizen => vec!["-Qua"],
            AurManagers::Pikaur => vec!["-Qua"],
            AurManagers::PamacCli => vec!["list", "-u", "-a"],
        };
    }

    pub fn install_args(&self) -> Vec<&'static str> {
        return match self {
            AurManagers::Yay => vec!["-S"],
            AurManagers::Paru => vec!["-S"],
            AurManagers::Trizen => vec!["-S"],
            AurManagers::Pikaur => vec!["-S"],
            AurManagers::PamacCli => vec!["install"],
        };
    }

    pub fn supports_noconfirm(&self) -> bool {
        return match self {
            AurManagers::PamacCli => false,
            _ => true,
        };
    }
}
