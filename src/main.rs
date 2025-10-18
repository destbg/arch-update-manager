mod constants;
mod helpers;
mod models;
mod ui;

use crate::ui::build_ui;
use gtk4::Application;
use gtk4::prelude::*;
use std::env;

fn main() {
    eprintln!("DISPLAY: {:?}", env::var("DISPLAY"));
    eprintln!("XDG_RUNTIME_DIR: {:?}", env::var("XDG_RUNTIME_DIR"));
    eprintln!(
        "DBUS_SESSION_BUS_ADDRESS: {:?}",
        env::var("DBUS_SESSION_BUS_ADDRESS")
    );

    if env::var("USER").unwrap_or_default() == "root"
        && env::var("DBUS_SESSION_BUS_ADDRESS").is_err()
    {
        eprintln!(
            "Running as root without D-Bus session, attempting to find active user session..."
        );

        let mut target_user = env::var("SUDO_USER").ok();

        if target_user.is_none() {
            if let Ok(output) = std::process::Command::new("who").output() {
                if let Ok(who_output) = String::from_utf8(output.stdout) {
                    for line in who_output.lines() {
                        if line.contains(":0") || line.contains("tty") {
                            if let Some(username) = line.split_whitespace().next() {
                                target_user = Some(username.to_string());
                                eprintln!("Found X session user: {}", username);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if target_user.is_none() {
            if let Ok(output) = std::process::Command::new("loginctl")
                .args(&["list-sessions", "--no-legend"])
                .output()
            {
                if let Ok(sessions_output) = String::from_utf8(output.stdout) {
                    for line in sessions_output.lines() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 && (parts[2] == "seat0" || parts[1] != "root") {
                            target_user = Some(parts[1].to_string());
                            eprintln!("Found session user: {}", parts[1]);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(user) = target_user {
            eprintln!("Using user: {}", user);

            if let Ok(output) = std::process::Command::new("id")
                .args(&["-u", &user])
                .output()
            {
                if let Ok(uid_str) = String::from_utf8(output.stdout) {
                    let uid = uid_str.trim();
                    eprintln!("Found user UID: {}", uid);

                    let user_home = format!("/home/{}", user);

                    unsafe {
                        if env::var("XDG_RUNTIME_DIR").is_err() {
                            let runtime_dir = format!("/run/user/{}", uid);
                            env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
                            eprintln!("Set XDG_RUNTIME_DIR to: {}", runtime_dir);
                        }

                        if env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
                            let dbus_address = format!("unix:path=/run/user/{}/bus", uid);
                            env::set_var("DBUS_SESSION_BUS_ADDRESS", &dbus_address);
                            eprintln!("Set DBUS_SESSION_BUS_ADDRESS to: {}", dbus_address);
                        }

                        if env::var("HOME").unwrap_or_default() == "/root" {
                            env::set_var("HOME", &user_home);
                            eprintln!("Set HOME to: {}", user_home);
                        }

                        if env::var("XDG_CONFIG_HOME").is_err() {
                            let config_home = format!("{}/.config", user_home);
                            env::set_var("XDG_CONFIG_HOME", &config_home);
                            eprintln!("Set XDG_CONFIG_HOME to: {}", config_home);
                        }

                        if env::var("XDG_DATA_HOME").is_err() {
                            let data_home = format!("{}/.local/share", user_home);
                            env::set_var("XDG_DATA_HOME", &data_home);
                            eprintln!("Set XDG_DATA_HOME to: {}", data_home);
                        }

                        if env::var("XDG_CACHE_HOME").is_err() {
                            let cache_home = format!("{}/.cache", user_home);
                            env::set_var("XDG_CACHE_HOME", &cache_home);
                            eprintln!("Set XDG_CACHE_HOME to: {}", cache_home);
                        }

                        if env::var("XAUTHORITY").is_err() {
                            let xauth_file = format!("{}/.Xauthority", user_home);
                            env::set_var("XAUTHORITY", &xauth_file);
                            eprintln!("Set XAUTHORITY to: {}", xauth_file);
                        }
                    }
                }
            }
        } else {
            eprintln!("Could not determine original user, D-Bus session may not work properly");
        }
    }

    gtk4::init().expect("Failed to initialize GTK4");

    let app = Application::builder()
        .application_id("com.destbg.arch-update-manager")
        .build();

    app.connect_activate(build_ui);

    eprintln!("Starting application run loop...");
    app.run();
}
