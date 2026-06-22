pub struct PkgbuildReview {
    pub package: String,
    pub diff: Option<String>,
    pub needs_review: bool,
    pub pkgbuild: Option<String>,
}
