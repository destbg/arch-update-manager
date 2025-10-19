mod constants;
mod helpers;
mod models;
mod ui;

use crate::constants::APP_ID;
use crate::ui::build_ui;
use gtk4::Application;
use gtk4::prelude::*;
use std::env;

fn main() {
    setup_user_environment();

    gtk4::init().expect("Failed to initialize GTK4");

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    app.run();
}

fn setup_user_environment() {
    let is_root = env::var("USER").unwrap_or_default() == "root";
    let no_dbus = env::var("DBUS_SESSION_BUS_ADDRESS").is_err();

    if !is_root || !no_dbus {
        return;
    }

    let target_user = find_target_user();
    if target_user.is_none() {
        return;
    }

    let user = target_user.unwrap();
    let uid = get_user_uid(&user);
    if uid.is_none() {
        return;
    }

    set_user_environment_vars(&user, &uid.unwrap());
}

fn find_target_user() -> Option<String> {
    if let Some(sudo_user) = env::var("SUDO_USER").ok() {
        return Some(sudo_user);
    }

    if let Some(user) = find_user_from_who() {
        return Some(user);
    }

    return find_user_from_loginctl();
}

fn find_user_from_who() -> Option<String> {
    let output = std::process::Command::new("who").output().ok()?;
    let who_output = String::from_utf8(output.stdout).ok()?;

    for line in who_output.lines() {
        if line.contains(":0") || line.contains("tty") {
            if let Some(username) = line.split_whitespace().next() {
                return Some(username.to_string());
            }
        }
    }
    return None;
}

fn find_user_from_loginctl() -> Option<String> {
    let output = std::process::Command::new("loginctl")
        .args(&["list-sessions", "--no-legend"])
        .output()
        .ok()?;

    let sessions_output = String::from_utf8(output.stdout).ok()?;

    for line in sessions_output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && (parts[2] == "seat0" || parts[1] != "root") {
            return Some(parts[1].to_string());
        }
    }
    return None;
}

fn get_user_uid(user: &str) -> Option<String> {
    let output = std::process::Command::new("id")
        .args(&["-u", user])
        .output()
        .ok()?;

    let uid_str = String::from_utf8(output.stdout).ok()?;
    return Some(uid_str.trim().to_string());
}

fn set_user_environment_vars(user: &str, uid: &str) {
    let user_home = format!("/home/{}", user);

    unsafe {
        if env::var("XDG_RUNTIME_DIR").is_err() {
            env::set_var("XDG_RUNTIME_DIR", format!("/run/user/{}", uid));
        }

        if env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            env::set_var(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path=/run/user/{}/bus", uid),
            );
        }

        if env::var("HOME").unwrap_or_default() == "/root" {
            env::set_var("HOME", &user_home);
        }

        if env::var("XDG_CONFIG_HOME").is_err() {
            env::set_var("XDG_CONFIG_HOME", format!("{}/.config", user_home));
        }

        if env::var("XDG_DATA_HOME").is_err() {
            env::set_var("XDG_DATA_HOME", format!("{}/.local/share", user_home));
        }

        if env::var("XDG_CACHE_HOME").is_err() {
            env::set_var("XDG_CACHE_HOME", format!("{}/.cache", user_home));
        }

        if env::var("XAUTHORITY").is_err() {
            env::set_var("XAUTHORITY", format!("{}/.Xauthority", user_home));
        }
    }
}
