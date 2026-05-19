use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Local};
use ksni::blocking::TrayMethods;
use ksni::menu::{StandardItem, SubMenu};
use ksni::{MenuItem, Status, ToolTip, Tray};
use signal_hook::consts::SIGUSR1;
use signal_hook::iterator::Signals;

use arch_update_manager::helpers::settings::{load_settings, reload_settings};
use arch_update_manager::models::tray_state::{TrayState, state_file};

const FALLBACK_POLL_INTERVAL: Duration = Duration::from_secs(300);
const ICON_NO_UPDATES: &str = "arch-update-manager";
const ICON_UPDATES_AVAILABLE: &str = "software-update-available-symbolic";

struct ArchUpdateTray {
    state: TrayState,
    expect_check_notification: Arc<AtomicBool>,
}

impl ArchUpdateTray {
    fn launch_main_app() {
        if let Err(e) = std::process::Command::new("pkexec")
            .arg("arch-update-manager")
            .spawn()
        {
            eprintln!("Failed to launch arch-update-manager: {}", e);
        }
    }

    fn run_check(&self) {
        self.expect_check_notification.store(true, Ordering::SeqCst);
        if let Err(e) = std::process::Command::new("systemctl")
            .args(["--user", "start", "arch-update-manager-check.service"])
            .status()
        {
            eprintln!("Failed to trigger check: {}", e);
        }
    }

    fn visible_total(&self) -> usize {
        let settings = load_settings();
        if settings.tray_only_favorites && settings.enable_favorites {
            return count_favorite_updates(&self.state, &settings.favorite_packages);
        }
        return self.state.total();
    }
}

fn count_favorite_updates(state: &TrayState, favorites: &[String]) -> usize {
    return state
        .packages
        .iter()
        .chain(state.aur.iter())
        .chain(state.flatpak.iter())
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or("");
            favorites.iter().any(|f| f == name)
        })
        .count();
}

impl Tray for ArchUpdateTray {
    fn id(&self) -> String {
        return "arch-update-manager-tray".into();
    }

    fn title(&self) -> String {
        return "Arch Update Manager".into();
    }

    fn icon_name(&self) -> String {
        if self.visible_total() == 0 {
            return ICON_NO_UPDATES.into();
        }
        return ICON_UPDATES_AVAILABLE.into();
    }

    fn status(&self) -> Status {
        if self.visible_total() > 0 {
            return Status::Active;
        }
        if load_settings().tray_always_visible {
            return Status::Active;
        }
        return Status::Passive;
    }

    fn tool_tip(&self) -> ToolTip {
        let title = match self.state.total() {
            0 => "System is up to date".to_string(),
            1 => "1 update available".to_string(),
            n => format!("{} updates available", n),
        };

        let description = match self.state.last_check {
            Some(t) => {
                let local: DateTime<Local> = t.into();
                format!("Last check: {}", local.format("%d %b %H:%M"))
            }
            None => "Last check: never".to_string(),
        };

        return ToolTip {
            title,
            description,
            ..Default::default()
        };
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        Self::launch_main_app();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let total = self.state.total();

        let count_label = match total {
            0 => "System is up to date".to_string(),
            1 => "1 update available".to_string(),
            n => format!("{} updates available", n),
        };

        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: count_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];

        if !self.state.packages.is_empty() {
            items.push(make_submenu(
                &format!("Packages ({})", self.state.packages.len()),
                &self.state.packages,
            ));
        }

        if !self.state.aur.is_empty() {
            items.push(make_submenu(
                &format!("AUR ({})", self.state.aur.len()),
                &self.state.aur,
            ));
        }

        if !self.state.flatpak.is_empty() {
            items.push(make_submenu(
                &format!("Flatpak ({})", self.state.flatpak.len()),
                &self.state.flatpak,
            ));
        }

        items.push(MenuItem::Separator);

        let last_check_label = match self.state.last_check {
            Some(t) => {
                let local: DateTime<Local> = t.into();
                format!("Last check: {}", local.format("%d %b %H:%M"))
            }
            None => "Last check: never".to_string(),
        };
        items.push(
            StandardItem {
                label: last_check_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);

        items.push(
            StandardItem {
                label: "Open Arch Update Manager".into(),
                activate: Box::new(|_: &mut Self| Self::launch_main_app()),
                ..Default::default()
            }
            .into(),
        );

        items.push(
            StandardItem {
                label: "Check for updates".into(),
                activate: Box::new(|s: &mut Self| s.run_check()),
                ..Default::default()
            }
            .into(),
        );

        items.push(
            StandardItem {
                label: "Exit".into(),
                activate: Box::new(|_: &mut Self| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        );

        return items;
    }
}

fn make_submenu(title: &str, entries: &[String]) -> MenuItem<ArchUpdateTray> {
    let submenu: Vec<MenuItem<ArchUpdateTray>> = entries
        .iter()
        .map(|entry| {
            StandardItem {
                label: entry.clone(),
                enabled: false,
                ..Default::default()
            }
            .into()
        })
        .collect();

    return SubMenu {
        label: title.into(),
        submenu,
        ..Default::default()
    }
    .into();
}

fn read_state(path: &PathBuf) -> TrayState {
    let Ok(content) = std::fs::read_to_string(path) else {
        return TrayState::default();
    };
    return serde_json::from_str(&content).unwrap_or_default();
}

fn main() {
    let path = match state_file() {
        Some(p) => p,
        None => {
            eprintln!("Could not determine state file location");
            std::process::exit(1);
        }
    };

    let initial_state = read_state(&path);
    let expect_check_notification = Arc::new(AtomicBool::new(false));

    let tray = ArchUpdateTray {
        state: initial_state.clone(),
        expect_check_notification: expect_check_notification.clone(),
    };

    let handle = match tray.spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to spawn tray: {}", e);
            std::process::exit(1);
        }
    };

    let last_seen = Arc::new(Mutex::new(initial_state));
    let path_clone = path.clone();
    let last_seen_clone = Arc::clone(&last_seen);
    let expect_check_for_thread = expect_check_notification.clone();

    let (tx, rx) = mpsc::channel::<()>();

    let tx_signal = tx.clone();
    thread::spawn(move || {
        let mut signals = match Signals::new([SIGUSR1]) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to install SIGUSR1 handler: {}", e);
                return;
            }
        };
        for _ in signals.forever() {
            let _ = tx_signal.send(());
        }
    });

    let tx_poll = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(FALLBACK_POLL_INTERVAL);
            let _ = tx_poll.send(());
        }
    });

    thread::spawn(move || {
        while rx.recv().is_ok() {
            reload_settings();
            let new_state = read_state(&path_clone);

            let (changed, prev_last_check, prev_total) = {
                let prev = last_seen_clone.lock().unwrap();
                (
                    !same_state(&prev, &new_state),
                    prev.last_check,
                    prev.total(),
                )
            };

            *last_seen_clone.lock().unwrap() = new_state.clone();
            handle.update(|t: &mut ArchUpdateTray| {
                t.state = new_state.clone();
            });

            let is_new_check = match (prev_last_check, new_state.last_check) {
                (Some(prev), Some(curr)) => curr > prev,
                (None, Some(_)) => true,
                _ => false,
            };

            if is_new_check {
                let manual = expect_check_for_thread.swap(false, Ordering::SeqCst);
                if manual {
                    fire_check_complete_notification(new_state.total());
                } else if changed
                    && prev_total == 0
                    && new_state.total() > 0
                    && load_settings().show_update_notifications
                {
                    fire_notification(new_state.total());
                }
            }
        }
    });

    loop {
        thread::park();
    }
}

fn same_state(a: &TrayState, b: &TrayState) -> bool {
    return a.last_check == b.last_check
        && a.packages == b.packages
        && a.aur == b.aur
        && a.flatpak == b.flatpak;
}

fn fire_notification(count: usize) {
    let body = match count {
        1 => "1 update available".to_string(),
        n => format!("{} updates available", n),
    };

    thread::spawn(move || {
        let result = notify_rust::Notification::new()
            .summary("Arch Updates Available")
            .body(&body)
            .icon("arch-update-manager")
            .appname("Arch Update Manager")
            .action("default", "Open")
            .action("open", "Open Update Manager")
            .show();

        match result {
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" || action == "open" {
                    ArchUpdateTray::launch_main_app();
                }
            }),
            Err(e) => eprintln!("Failed to show notification: {}", e),
        }
    });
}

fn fire_check_complete_notification(count: usize) {
    let (summary, body) = match count {
        0 => ("Check Complete", "System is up to date".to_string()),
        1 => ("Arch Updates Available", "1 update available".to_string()),
        n => (
            "Arch Updates Available",
            format!("{} updates available", n),
        ),
    };

    thread::spawn(move || {
        let mut notification = notify_rust::Notification::new();
        notification
            .summary(summary)
            .body(&body)
            .icon("arch-update-manager")
            .appname("Arch Update Manager");

        let result = if count > 0 {
            notification
                .action("default", "Open")
                .action("open", "Open Update Manager")
                .show()
        } else {
            notification.show()
        };

        match result {
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" || action == "open" {
                    ArchUpdateTray::launch_main_app();
                }
            }),
            Err(e) => eprintln!("Failed to show notification: {}", e),
        }
    });
}
