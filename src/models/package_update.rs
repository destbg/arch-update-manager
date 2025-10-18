#[derive(Clone, Debug)]
pub struct PackageUpdate {
    pub repository: String,
    pub selected: bool,
    pub name: String,
    pub description: String,
    pub current_version: String,
    pub new_version: String,
    pub size: i64,
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
        }
    }
}
