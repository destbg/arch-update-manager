pub enum DiffRow {
    File { path: String, change: &'static str },
    Hunk { context: String },
    Context(String),
    Added(String),
    Removed(String),
}
