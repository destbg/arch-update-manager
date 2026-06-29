use arch_update_manager::constants::APP_ID;
use arch_update_manager::helpers::logger;
use arch_update_manager::helpers::settings::load_settings;
use arch_update_manager::log_info;
use arch_update_manager::ui::build_ui;
use gtk4::Application;
use gtk4::prelude::*;
use std::env;
use std::process::Command;

fn main() {
    setup_user_environment();

    logger::init();
    logger::cleanup_old_logs(load_settings().log_retention_days);
    log_info!("application starting");

    gtk4::init().expect("Failed to initialize GTK4");

    apply_system_theme();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(|app| {
        if !is_running_as_root() {
            log_info!("not running as root, showing dialog");
            show_not_root_dialog_and_quit(app);
            return;
        }
        log_info!("building UI");
        build_ui(app);
    });

    app.run();
    log_info!("application exiting");
}

fn is_running_as_root() -> bool {
    return unsafe { libc::geteuid() } == 0;
}

fn show_not_root_dialog_and_quit(app: &Application) {
    let alert = gtk4::AlertDialog::builder()
        .modal(true)
        .message("Admin rights needed")
        .detail(
            "Arch Update Manager needs to run with admin rights. Please launch it from your application menu, or run it with pkexec or sudo.",
        )
        .buttons(["OK"])
        .build();

    let guard = app.hold();
    let app_clone = app.clone();
    alert.choose(None::<&gtk4::Window>, gio::Cancellable::NONE, move |_| {
        drop(guard);
        app_clone.quit();
    });
}

fn apply_system_theme() {
    let Some(settings) = gtk4::Settings::default() else {
        return;
    };

    if let Some(theme) = get_system_gtk_theme() {
        settings.set_gtk_theme_name(Some(&theme));
    }

    if let Some(prefers_dark) = detect_prefers_dark() {
        settings.set_gtk_application_prefer_dark_theme(prefers_dark);
    }
}

fn get_system_gtk_theme() -> Option<String> {
    if let Ok(theme_env) = env::var("GTK_THEME") {
        return Some(theme_env);
    }

    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if let Ok(val) = String::from_utf8(output.stdout) {
            let theme = val.trim().trim_matches('\'').to_string();
            if !theme.is_empty() {
                return Some(theme);
            }
        }
    }

    return None;
}

fn detect_prefers_dark() -> Option<bool> {
    if let Ok(theme_env) = env::var("GTK_THEME") {
        let t = theme_env.to_lowercase();
        return Some(t.contains("dark"));
    }

    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if let Ok(val) = String::from_utf8(output.stdout) {
            let v = val.trim().to_lowercase();
            if v.contains("prefer-dark") {
                return Some(true);
            }
            if v.contains("default") || v.contains("prefer-light") {
                return Some(false);
            }
        }
    }

    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if let Ok(val) = String::from_utf8(output.stdout) {
            if val.trim().to_lowercase().contains("dark") {
                return Some(true);
            }
        }
    }

    return None;
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
    let output = Command::new("who").output().ok()?;
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
    let output = Command::new("loginctl")
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
    let output = Command::new("id").args(&["-u", user]).output().ok()?;

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
