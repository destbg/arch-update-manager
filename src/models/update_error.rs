#[derive(Debug)]
pub enum UpdateError {
    CommandFailed(String),
    IoError(String),
    SyncFailed(String),
}
