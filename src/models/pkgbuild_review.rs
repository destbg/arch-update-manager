pub struct PkgbuildReview {
    pub package: String,
    pub old_content: Option<String>,
    pub old_label: String,
    pub new_content: String,
    pub new_label: String,
}
