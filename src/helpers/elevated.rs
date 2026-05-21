use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

pub fn get_original_user() -> Option<String> {
    if let Ok(user) = env::var("SUDO_USER") {
        if !user.is_empty() && user != "root" {
            return Some(user);
        }
    }

    if let Ok(uid) = env::var("PKEXEC_UID") {
        if let Ok(uid_num) = uid.parse::<u32>() {
            if uid_num != 0 {
                if let Ok(output) = Command::new("id").args(&["-un", &uid]).output() {
                    let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !username.is_empty() {
                        return Some(username);
                    }
                }
            }
        }
    }

    if let Ok(output) = Command::new("who").output() {
        if let Ok(who_output) = String::from_utf8(output.stdout) {
            for line in who_output.lines() {
                if line.contains(":0") || line.contains("tty") {
                    if let Some(username) = line.split_whitespace().next() {
                        if username != "root" {
                            return Some(username.to_string());
                        }
                    }
                }
            }
        }
    }

    return None;
}

pub fn open_url_as_user(url: &str) {
    if !is_safe_url(url) {
        return;
    }

    spawn_as_user_or_root("xdg-open", &[url]);
}

pub fn spawn_as_user_or_root(program: &str, args: &[&str]) {
    if let Some(user) = get_original_user() {
        let preserve = "DISPLAY,WAYLAND_DISPLAY,DBUS_SESSION_BUS_ADDRESS,XDG_RUNTIME_DIR,XDG_SESSION_TYPE,XDG_CURRENT_DESKTOP,XDG_DATA_DIRS,XAUTHORITY,HOME";

        let mut sudo_args: Vec<&str> = vec!["-u", &user];
        let preserve_arg = format!("--preserve-env={}", preserve);
        sudo_args.push(&preserve_arg);
        sudo_args.push(program);
        for arg in args {
            sudo_args.push(arg);
        }

        if Command::new("sudo").args(&sudo_args).spawn().is_ok() {
            return;
        }
    }

    let _ = Command::new(program).args(args).spawn();
}

static USER_IDS_CACHE: OnceLock<Option<(u32, u32)>> = OnceLock::new();

pub fn get_original_user_ids() -> Option<(u32, u32)> {
    return *USER_IDS_CACHE.get_or_init(|| {
        let user = get_original_user()?;
        let uid: u32 = Command::new("id")
            .args(["-u", &user])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())?;
        let gid: u32 = Command::new("id")
            .args(["-g", &user])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())?;
        return Some((uid, gid));
    });
}

pub fn chown_to_user(path: &Path) {
    let Some((uid, gid)) = get_original_user_ids() else {
        return;
    };
    let _ = std::os::unix::fs::chown(path, Some(uid), Some(gid));
}

fn is_safe_url(url: &str) -> bool {
    const SAFE_SCHEMES: &[&str] = &["http://", "https://", "appstream://"];
    for scheme in SAFE_SCHEMES {
        if url.starts_with(scheme) {
            return true;
        }
    }
    return false;
}
