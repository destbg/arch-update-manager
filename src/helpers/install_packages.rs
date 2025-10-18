use anyhow::Result;
use std::io::Write;

use crate::helpers::terminal::launch_terminal_with_command;

pub fn install_selected_packages(package_names: Vec<String>) -> Result<()> {
    if package_names.is_empty() {
        return Err(anyhow::anyhow!("No packages selected for installation"));
    }

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("arch_update_manager_install.sh");
    let completion_marker = temp_dir.join("arch_update_manager_install_complete.marker");

    let _ = std::fs::remove_file(&completion_marker);

    let mut command_args = vec!["sudo".to_string(), "pacman".to_string(), "-S".to_string()];
    command_args.extend(package_names);
    let command_str = command_args.join(" ");

    let script_content = format!(
        "#!/bin/bash\n\
        echo 'Installing packages...'\n\
        {}\n\
        installation_result=$?\n\
        echo 'Installation completed with exit code: $installation_result'\n\
        echo $installation_result > '{}'\n\
        if [ $installation_result -eq 0 ]; then\n\
            echo 'Package installation successful!'\n\
        else\n\
            echo 'Package installation failed!'\n\
        fi\n\
        read -p 'Press Enter to continue...'\n",
        command_str,
        completion_marker.to_string_lossy()
    );

    let mut file = std::fs::File::create(&script_path)?;
    file.write_all(script_content.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms)?;
    }

    let full_command = format!("bash '{}'", script_path.to_string_lossy());
    return launch_terminal_with_command(&full_command);
}

pub fn check_installation_status() -> Option<bool> {
    let temp_dir = std::env::temp_dir();
    let completion_marker = temp_dir.join("arch_update_manager_install_complete.marker");

    if completion_marker.exists() {
        if let Ok(content) = std::fs::read_to_string(&completion_marker) {
            if let Ok(exit_code) = content.trim().parse::<i32>() {
                let _ = std::fs::remove_file(&completion_marker);
                return Some(exit_code == 0);
            }
        }
        let _ = std::fs::remove_file(&completion_marker);
        return Some(true);
    }

    return None;
}
