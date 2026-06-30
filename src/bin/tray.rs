use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Local};
use ksni::blocking::TrayMethods;
use ksni::menu::{StandardItem, SubMenu};
use ksni::{MenuItem, Status, ToolTip, Tray};
use signal_hook::consts::SIGUSR1;
use signal_hook::iterator::Signals;

use arch_update_manager::helpers::settings::{load_settings, reload_settings, save_settings};
use arch_update_manager::helpers::snooze::{clear_snooze, current_snooze_until, set_snooze};
use arch_update_manager::helpers::tray_state::state_file;
use arch_update_manager::models::app_settings::AppSettings;
use arch_update_manager::models::tray_state::TrayState;

static REFRESH_TX: OnceLock<mpsc::Sender<()>> = OnceLock::new();

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
        if let Err(e) = std::process::Command::new("arch-update-manager-check")
            .arg("--manual")
            .status()
        {
            eprintln!("Failed to trigger check: {}", e);
        }
    }

    fn visible_total(&self) -> usize {
        let settings = load_settings();
        let filter_by_favorites = settings.enable_favorites
            && (settings.tray_only_favorites || settings.tray_menu_only_favorites);
        if filter_by_favorites {
            return count_favorite_updates(&self.state, &settings);
        }
        return self.state.total();
    }

    fn prompt_remove_favorite(&self, package: String) {
        if package.is_empty() {
            return;
        }

        thread::spawn(move || {
            if is_main_app_running() {
                let text = format!(
                    "Close Arch Update Manager first to change favorites.\n\n\"{}\" was not changed.",
                    package
                );
                if !show_warning_dialog("Arch Update Manager is open", &text) {
                    notify_app_running(&package);
                }
                return;
            }

            let text = format!(
                "Remove \"{}\" from favorites?\n\nIt will no longer show in the tray.",
                package
            );
            match show_question_dialog("Remove from favorites?", &text) {
                Some(true) => remove_favorite(&package),
                Some(false) => {}
                None => confirm_remove_via_notification(package),
            }
        });
    }
}

fn remove_favorite(package: &str) {
    let mut settings = reload_settings();
    settings.set_favorite(package, false);
    if let Err(e) = save_settings(&settings) {
        eprintln!("Failed to save settings after removing favorite: {}", e);
        return;
    }
    request_refresh();
}

fn confirm_remove_via_notification(package: String) {
    let body = format!(
        "Remove \"{}\" from favorites? It will no longer show in the tray.",
        package
    );
    let result = notify_rust::Notification::new()
        .summary("Remove from favorites?")
        .body(&body)
        .icon("arch-update-manager")
        .appname("Arch Update Manager")
        .action("remove", "Remove")
        .action("default", "Cancel")
        .show();

    match result {
        Ok(handle) => handle.wait_for_action(|action| {
            if action == "remove" {
                remove_favorite(&package);
            }
        }),
        Err(e) => eprintln!("Failed to show confirm notification: {}", e),
    }
}

fn notify_app_running(package: &str) {
    let body = format!(
        "Close Arch Update Manager first to change favorites. \"{}\" was not changed.",
        package
    );
    let _ = notify_rust::Notification::new()
        .summary("Arch Update Manager is open")
        .body(&body)
        .icon("arch-update-manager")
        .appname("Arch Update Manager")
        .show();
}

fn show_question_dialog(title: &str, text: &str) -> Option<bool> {
    if which("zenity") {
        return run_dialog(
            "zenity",
            &[
                "--question",
                "--title",
                title,
                "--text",
                text,
                "--ok-label",
                "Remove",
                "--cancel-label",
                "Cancel",
            ],
        );
    }
    if which("kdialog") {
        return run_dialog("kdialog", &["--title", title, "--yesno", text]);
    }
    return None;
}

fn show_warning_dialog(title: &str, text: &str) -> bool {
    if which("zenity") {
        return run_dialog("zenity", &["--warning", "--title", title, "--text", text]).is_some();
    }
    if which("kdialog") {
        return run_dialog("kdialog", &["--title", title, "--sorry", text]).is_some();
    }
    return false;
}

fn run_dialog(program: &str, args: &[&str]) -> Option<bool> {
    return match std::process::Command::new(program).args(args).status() {
        Ok(status) => Some(status.success()),
        Err(e) => {
            eprintln!("Failed to run {}: {}", program, e);
            None
        }
    };
}

fn which(program: &str) -> bool {
    return std::process::Command::new("which")
        .arg(program)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
}

fn is_main_app_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        let comm = comm.trim_end();
        if comm.is_empty() || !"arch-update-manager".starts_with(comm) {
            continue;
        }

        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let Some(arg0) = cmdline.split(|b| *b == 0).next() else {
            continue;
        };
        if arg0.is_empty() {
            continue;
        }

        let arg0 = String::from_utf8_lossy(arg0);
        let base = arg0.rsplit('/').next().unwrap_or(&arg0);
        if base == "arch-update-manager" {
            return true;
        }
    }

    return false;
}

fn request_refresh() {
    if let Some(tx) = REFRESH_TX.get() {
        let _ = tx.send(());
    }
}

fn package_name_from_entry(entry: &str) -> &str {
    return entry.split_whitespace().next().unwrap_or("");
}

fn count_favorite_updates(state: &TrayState, settings: &AppSettings) -> usize {
    return state
        .packages
        .iter()
        .chain(state.aur.iter())
        .chain(state.flatpak.iter())
        .chain(state.appimage.iter())
        .filter(|line| settings.is_favorite(package_name_from_entry(line)))
        .count();
}

fn filter_favorite_entries(entries: &[String], settings: &AppSettings) -> Vec<String> {
    return entries
        .iter()
        .filter(|line| settings.is_favorite(package_name_from_entry(line)))
        .cloned()
        .collect();
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
        let title = if let Some(until) = current_snooze_until() {
            let local: DateTime<Local> = until.into();
            format!("Snoozed until {}", local.format("%d %b %H:%M"))
        } else {
            match self.state.total() {
                0 => "System is up to date".to_string(),
                1 => "1 update available".to_string(),
                n => format!("{} updates available", n),
            }
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
        let settings = load_settings();
        let filter_to_favorites = settings.tray_menu_only_favorites && settings.enable_favorites;

        let packages = if filter_to_favorites {
            filter_favorite_entries(&self.state.packages, &settings)
        } else {
            self.state.packages.clone()
        };
        let aur = if filter_to_favorites {
            filter_favorite_entries(&self.state.aur, &settings)
        } else {
            self.state.aur.clone()
        };
        let flatpak = if filter_to_favorites {
            filter_favorite_entries(&self.state.flatpak, &settings)
        } else {
            self.state.flatpak.clone()
        };
        let appimage = if filter_to_favorites {
            filter_favorite_entries(&self.state.appimage, &settings)
        } else {
            self.state.appimage.clone()
        };

        let snooze_until = current_snooze_until();
        let visible_total = packages.len() + aur.len() + flatpak.len() + appimage.len();
        let count_label = if let Some(until) = snooze_until {
            let local: DateTime<Local> = until.into();
            format!("Snoozed until {}", local.format("%d %b %H:%M"))
        } else {
            match visible_total {
                0 => "System is up to date".to_string(),
                1 => "1 update available".to_string(),
                n => format!("{} updates available", n),
            }
        };

        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: count_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];

        if !packages.is_empty() {
            items.push(make_submenu(
                &format!("Packages ({})", packages.len()),
                &packages,
                filter_to_favorites,
            ));
        }

        if !aur.is_empty() {
            items.push(make_submenu(
                &format!("AUR ({})", aur.len()),
                &aur,
                filter_to_favorites,
            ));
        }

        if !flatpak.is_empty() {
            items.push(make_submenu(
                &format!("Flatpak ({})", flatpak.len()),
                &flatpak,
                filter_to_favorites,
            ));
        }

        if !appimage.is_empty() {
            items.push(make_submenu(
                &format!("AppImage ({})", appimage.len()),
                &appimage,
                filter_to_favorites,
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
                label: "Open Update Manager".into(),
                activate: Box::new(|_: &mut Self| Self::launch_main_app()),
                ..Default::default()
            }
            .into(),
        );

        items.push(
            StandardItem {
                label: "Check for updates".into(),
                enabled: snooze_until.is_none(),
                activate: Box::new(|s: &mut Self| s.run_check()),
                ..Default::default()
            }
            .into(),
        );

        items.push(build_snooze_menu(snooze_until));

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

fn build_snooze_menu(snooze_until: Option<DateTime<chrono::Utc>>) -> MenuItem<ArchUpdateTray> {
    let mut submenu: Vec<MenuItem<ArchUpdateTray>> = Vec::new();

    if let Some(until) = snooze_until {
        let local: DateTime<Local> = until.into();
        submenu.push(
            StandardItem {
                label: format!("Cancel snooze (until {})", local.format("%d %b %H:%M")),
                activate: Box::new(|_: &mut ArchUpdateTray| {
                    if let Err(e) = clear_snooze() {
                        eprintln!("Failed to clear snooze: {}", e);
                        return;
                    }
                    request_refresh();
                }),
                ..Default::default()
            }
            .into(),
        );
        submenu.push(MenuItem::Separator);
    }

    for hours in [1u32, 4, 8, 24] {
        let label = if hours == 1 {
            "1 hour".to_string()
        } else {
            format!("{} hours", hours)
        };
        submenu.push(
            StandardItem {
                label,
                activate: Box::new(move |_: &mut ArchUpdateTray| {
                    if let Err(e) = set_snooze(hours) {
                        eprintln!("Failed to snooze: {}", e);
                        return;
                    }
                    request_refresh();
                }),
                ..Default::default()
            }
            .into(),
        );
    }

    return SubMenu {
        label: "Snooze".into(),
        submenu,
        ..Default::default()
    }
    .into();
}

fn make_submenu(title: &str, entries: &[String], clickable: bool) -> MenuItem<ArchUpdateTray> {
    let submenu: Vec<MenuItem<ArchUpdateTray>> = entries
        .iter()
        .map(|entry| {
            let package = package_name_from_entry(entry).to_string();
            StandardItem {
                label: entry.clone(),
                enabled: clickable,
                activate: Box::new(move |tray: &mut ArchUpdateTray| {
                    tray.prompt_remove_favorite(package.clone());
                }),
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

    thread::spawn(|| {
        if let Err(e) = std::process::Command::new("arch-update-manager-check").status() {
            eprintln!("Failed to run initial check on tray startup: {}", e);
        }
    });

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
    let _ = REFRESH_TX.set(tx.clone());

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
            let settings = reload_settings();
            let new_state = read_state(&path_clone);

            let only_favorites = settings.enable_favorites
                && (settings.tray_only_favorites || settings.tray_menu_only_favorites);

            let (changed, prev_last_check, prev_relevant_total) = {
                let prev = last_seen_clone.lock().unwrap();
                let prev_count = if only_favorites {
                    count_favorite_updates(&prev, &settings)
                } else {
                    prev.total()
                };
                (!same_state(&prev, &new_state), prev.last_check, prev_count)
            };

            let new_relevant_total = if only_favorites {
                count_favorite_updates(&new_state, &settings)
            } else {
                new_state.total()
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

            let snoozed = current_snooze_until().is_some();
            if is_new_check && !snoozed {
                let manual = expect_check_for_thread.swap(false, Ordering::SeqCst);
                if manual {
                    fire_check_complete_notification(new_relevant_total);
                } else if changed
                    && prev_relevant_total == 0
                    && new_relevant_total > 0
                    && settings.show_update_notifications
                {
                    fire_notification(new_relevant_total);
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
        && a.flatpak == b.flatpak
        && a.appimage == b.appimage;
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
        n => ("Arch Updates Available", format!("{} updates available", n)),
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
