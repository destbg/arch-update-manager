#[derive(Clone)]
pub struct SnapshotGroup {
    pub enable_check: gtk4::CheckButton,
    pub provider_combo: gtk4::DropDown,
    pub retention_count_spin: gtk4::SpinButton,
    pub retention_period_combo: gtk4::DropDown,
    pub retention_count_box: gtk4::Box,
    pub retention_period_box: gtk4::Box,
    pub deletion_info_label: gtk4::Label,
    pub snap_pac_info: gtk4::Label,
    pub has_timeshift: bool,
    pub has_snapper: bool,
    pub snap_pac_installed: bool,
}
