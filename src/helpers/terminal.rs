use anyhow::Result;
use std::process::Command;

pub fn launch_terminal_with_command(command: &str) -> Result<()> {
    let terminals = [
        (
            "gnome-terminal",
            vec!["--geometry=80x24", "--", "bash", "-c", command],
        ),
        (
            "konsole",
            vec!["--geometry", "80x24", "-e", "bash", "-c", command],
        ),
        (
            "cosmic-term",
            vec!["--geometry", "80x24", "-e", "bash", "-c", command],
        ),
        (
            "xfce4-terminal",
            vec!["--geometry=80x24", "-e", "bash", "-c", command],
        ),
        (
            "alacritty",
            vec![
                "--option",
                "window.dimensions.columns=80",
                "--option",
                "window.dimensions.lines=24",
                "-e",
                "bash",
                "-c",
                command,
            ],
        ),
        (
            "kitty",
            vec![
                "--override",
                "initial_window_width=80c",
                "--override",
                "initial_window_height=24c",
                "bash",
                "-c",
                command,
            ],
        ),
        (
            "xterm",
            vec!["-geometry", "80x24", "-e", "bash", "-c", command],
        ),
    ];

    let mut last_error = None;

    for (terminal, args) in &terminals {
        if let Err(_) = Command::new("which").arg(terminal).output() {
            continue;
        }

        let result = Command::new(terminal).args(args).spawn();

        match result {
            Ok(mut child) => {
                println!("Opened {} with command: {}", terminal, command);

                std::thread::sleep(std::time::Duration::from_millis(100));

                match child.try_wait() {
                    Ok(Some(exit_status)) => {
                        if !exit_status.success() {
                            eprintln!("Terminal {} exited with error: {}", terminal, exit_status);
                            last_error = Some(format!("Terminal {} failed to start", terminal));
                            continue;
                        }
                    }
                    Ok(None) => {
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("Error checking terminal status: {}", e);
                        last_error = Some(format!("Error with terminal {}: {}", terminal, e));
                        continue;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to launch {}: {}", terminal, e);
                last_error = Some(format!("Failed to launch {}: {}", terminal, e));
                continue;
            }
        }
    }

    println!("All terminal emulators failed, trying fallback methods...");

    return Err(anyhow::anyhow!(
        "No suitable terminal emulator found. Last error: {}",
        last_error.unwrap_or_else(|| "Unknown error".to_string())
    ));
}
