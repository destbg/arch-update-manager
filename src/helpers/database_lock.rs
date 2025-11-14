use std::path::Path;

pub fn remove_database_lock() -> Result<(), String> {
    let lock_path = "/var/lib/pacman/db.lck";

    if !Path::new(lock_path).exists() {
        return Err("The database lock file does not exist.".to_string());
    }

    return match std::fs::remove_file(lock_path) {
        Ok(()) => Ok(()),
        Err(e) => Err(format!("Failed to remove database lock: {}", e)),
    };
}

pub fn is_lock_error(error_message: &str) -> bool {
    return error_message
        .to_lowercase()
        .contains("unable to lock database")
        || error_message.contains("db.lck");
}
