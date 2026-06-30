use crate::constants::TIMESHIFT_COMMENT;
use crate::helpers::appimage::build_appimage_update_commands;
use crate::helpers::aur::install_aur_packages;
use crate::helpers::disk_space::available_bytes;
use crate::helpers::elevated::open_url_as_user;
use crate::helpers::flatpak::build_flatpak_update_command;
use crate::helpers::get_navigation_stack::get_navigation_stack;
use crate::helpers::pacman_repos::get_repository_groups;
use crate::helpers::settings::load_settings;
use crate::helpers::snapper::{
    build_snapper_snapshot_command, is_snap_pac_installed, is_snapper_installed,
};
use crate::helpers::terminal::spawn_terminal;
use crate::helpers::timeshift::{cleanup_timeshift_snapshots, create_timeshift_snapshot};
use crate::log_info;
use crate::models::package_object::PackageUpdateObject;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;
use crate::ui::dialogs::{
    create_progress_dialog, show_confirm_dialog, show_error_dialog, show_partial_upgrade_dialog,
};
use crate::ui::history_dialog::show_history_dialog;
use crate::ui::main_window::{find_favorites_column, find_package_store, load_packages};
use crate::ui::package_list::{save_unselected_from_store, update_statusbar};
use crate::ui::settings_dialog::show_settings_dialog;
use crate::ui::vulnerabilities_dialog::show_vulnerabilities_dialog;
use gio::ListStore;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, Box as GtkBox, Button, ColumnView, FilterListModel, Frame, Image, Label,
    Orientation, Paned, ScrolledWindow, Separator, SingleSelection, SortListModel, Stack,
};
use shlex::try_quote as quote;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const DISK_SAFETY_MARGIN: i64 = 512 * 1024 * 1024;

pub fn create_toolbar(show_settings_button: bool) -> GtkBox {
    let toolbar_container = GtkBox::new(Orientation::Vertical, 6);
    toolbar_container.set_margin_start(6);
    toolbar_container.set_margin_end(6);
    toolbar_container.set_margin_top(6);
    toolbar_container.set_margin_bottom(6);

    let toolbar = GtkBox::new(Orientation::Horizontal, 6);

    let clear_btn = Button::new();
    clear_btn.add_css_class("destructive-action");
    clear_btn.set_child(Some(&create_button_content("edit-clear", "Clear")));
    clear_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            log_info!("toolbar: Clear clicked");
            if let Some((store, statusbar)) = find_store_and_statusbar(&toolbar) {
                clear_all_selections(&store, &statusbar);
            }
        }
    ));
    toolbar.append(&clear_btn);

    let select_all_btn = Button::new();
    select_all_btn.set_child(Some(&create_button_content(
        "edit-select-all",
        "Select All",
    )));
    select_all_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            log_info!("toolbar: Select All clicked");
            if let Some((store, statusbar)) = find_store_and_statusbar(&toolbar) {
                select_all_packages(&store, &statusbar);
            }
        }
    ));
    toolbar.append(&select_all_btn);

    let separator = Separator::new(Orientation::Vertical);
    toolbar.append(&separator);

    let refresh_btn = Button::new();
    refresh_btn.set_child(Some(&create_button_content("view-refresh", "Refresh")));

    refresh_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            log_info!("toolbar: Refresh clicked");
            let Some((stack, content_box, window)) = get_navigation_stack(&toolbar) else {
                return;
            };

            stack.set_visible_child_name("loading");
            load_packages(stack, content_box, window);
        }
    ));

    toolbar.append(&refresh_btn);

    let separator2 = Separator::new(Orientation::Vertical);
    toolbar.append(&separator2);

    let install_btn = Button::new();
    install_btn.add_css_class("suggested-action");
    install_btn.set_child(Some(&create_button_content(
        "system-software-install",
        "Install Updates",
    )));
    install_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            log_info!("toolbar: Install Updates clicked");
            if let Some((store, _statusbar)) = find_store_and_statusbar(&toolbar) {
                if let Some(window) = toolbar.root().and_downcast::<ApplicationWindow>() {
                    let settings = load_settings();
                    let create_snapshot = settings.create_timeshift_snapshot;
                    let create_snapper = settings.create_snapper_snapshot
                        && is_snapper_installed()
                        && !is_snap_pac_installed();

                    let mut snapshot_note = String::new();
                    if create_snapshot {
                        snapshot_note.push_str("\nA Timeshift snapshot will be created.");
                    }
                    if create_snapper {
                        snapshot_note.push_str("\nA Snapper snapshot will be created.");
                    }

                    let disk_prefix = disk_space_warning(&store)
                        .map(|w| format!("{}\n\n", w))
                        .unwrap_or_default();

                    let (selected_core, total_core) = count_core_updates(&store);
                    let partial = selected_core > 0 && selected_core < total_core;

                    if partial {
                        let message = format!(
                            "{}You selected {} of {} core package updates.\n\nInstalling only some of them is a partial upgrade.\nThis can sometimes cause errors or leave a package broken.{}",
                            disk_prefix, selected_core, total_core, snapshot_note
                        );
                        let store_full = store.clone();
                        let window_full = window.clone();
                        let store_selected = store.clone();
                        let window_selected = window.clone();
                        show_partial_upgrade_dialog(
                            &window,
                            &message,
                            move || {
                                log_info!("partial upgrade: full upgrade chosen");
                                select_all_official(&store_full);
                                run_install(&store_full, &window_full, create_snapshot, create_snapper);
                            },
                            move || {
                                log_info!("partial upgrade: install selected anyway");
                                run_install(
                                    &store_selected,
                                    &window_selected,
                                    create_snapshot,
                                    create_snapper,
                                );
                            },
                        );
                    } else {
                        let message = format!("{}Install selected updates?{}", disk_prefix, snapshot_note);
                        let store = store.clone();
                        let window_confirm = window.clone();
                        show_confirm_dialog(
                            &window,
                            "Confirm Installation",
                            &message,
                            "Install",
                            move |accepted| {
                                if accepted {
                                    log_info!("install confirmation accepted");
                                    run_install(
                                        &store,
                                        &window_confirm,
                                        create_snapshot,
                                        create_snapper,
                                    );
                                } else {
                                    log_info!("install confirmation dismissed");
                                }
                            },
                        );
                    }
                }
            }
        }
    ));
    toolbar.append(&install_btn);

    if show_settings_button {
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        toolbar.append(&spacer);

        let separator3 = Separator::new(Orientation::Vertical);
        toolbar.append(&separator3);

        let vulnerabilities_btn = Button::new();
        vulnerabilities_btn.set_child(Some(&create_button_content(
            "security-high-symbolic",
            "Security",
        )));
        vulnerabilities_btn.set_tooltip_text(Some("Open vulnerabilities (no fix available)"));
        vulnerabilities_btn.connect_clicked(clone!(
            #[weak]
            toolbar,
            move |_| {
                log_info!("toolbar: Security clicked");
                if let Some(window) = toolbar.root().and_downcast::<ApplicationWindow>() {
                    show_vulnerabilities_dialog(&window);
                }
            }
        ));
        toolbar.append(&vulnerabilities_btn);

        let history_btn = Button::new();
        history_btn.set_child(Some(&create_button_content(
            "document-open-recent-symbolic",
            "History",
        )));
        history_btn.set_tooltip_text(Some("Update history"));
        history_btn.connect_clicked(clone!(
            #[weak]
            toolbar,
            move |_| {
                log_info!("toolbar: History clicked");
                if let Some(window) = toolbar.root().and_downcast::<ApplicationWindow>() {
                    show_history_dialog(&window);
                }
            }
        ));
        toolbar.append(&history_btn);

        let news_btn = Button::new();
        news_btn.set_child(Some(&create_button_content(
            "application-rss+xml-symbolic",
            "News",
        )));
        news_btn.set_tooltip_text(Some("Arch Linux News"));
        news_btn.connect_clicked(|_| {
            log_info!("toolbar: News clicked");
            open_url_as_user("https://archlinux.org/news/");
        });
        toolbar.append(&news_btn);

        let settings_btn = Button::new();
        settings_btn.set_child(Some(&create_button_content(
            "preferences-system-symbolic",
            "Settings",
        )));
        settings_btn.set_tooltip_text(Some("Settings"));
        settings_btn.connect_clicked(clone!(
            #[weak]
            toolbar,
            move |_| {
                log_info!("toolbar: Settings clicked");
                if let Some(window) = toolbar.root().and_downcast::<ApplicationWindow>() {
                    let settings = load_settings();
                    let favorites_column = find_favorites_column(&window);
                    let package_store = find_package_store(&window);
                    show_settings_dialog(&window, &settings, favorites_column, package_store);
                }
            }
        ));
        toolbar.append(&settings_btn);
    }

    toolbar_container.append(&toolbar);

    let install_btn_for_focus = install_btn.clone();
    glib::idle_add_local_once(move || {
        install_btn_for_focus.grab_focus();
    });

    return toolbar_container;
}

fn find_store_and_statusbar(toolbar: &GtkBox) -> Option<(ListStore, Label)> {
    let Some((_, content_box, _)) = get_navigation_stack(toolbar) else {
        return None;
    };

    let Some(paned) = content_box
        .last_child()
        .and_then(|child| child.prev_sibling())
        .and_downcast::<Paned>()
    else {
        return None;
    };

    let Some(scrolled) = paned.start_child().and_downcast::<ScrolledWindow>() else {
        return None;
    };

    let Some(column_view) = scrolled.child().and_downcast::<ColumnView>() else {
        return None;
    };

    let Some(selection_model) = column_view.model() else {
        return None;
    };

    let Some(list_store) = selection_model
        .downcast_ref::<SingleSelection>()
        .and_then(|sm| sm.model())
        .and_then(|m| m.downcast::<FilterListModel>().ok())
        .and_then(|fm| fm.model())
        .and_then(|m| m.downcast::<SortListModel>().ok())
        .and_then(|sm| sm.model())
        .and_downcast::<ListStore>()
    else {
        return None;
    };

    let Some(statusbar) = content_box.last_child().and_downcast::<Label>() else {
        return None;
    };

    return Some((list_store, statusbar));
}

fn clear_all_selections(store: &ListStore, statusbar: &Label) {
    let n_items = store.n_items();
    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            item.set_selected(false);
        }
    }
    let items: Vec<PackageUpdateObject> = (0..n_items)
        .filter_map(|i| store.item(i).and_downcast::<PackageUpdateObject>())
        .collect();

    store.remove_all();
    for item in items {
        store.append(&item);
    }

    update_statusbar(statusbar, store);
    save_unselected_from_store(store);
}

fn select_all_packages(store: &ListStore, statusbar: &Label) {
    let n_items = store.n_items();
    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            item.set_selected(true);
        }
    }
    let items: Vec<PackageUpdateObject> = (0..n_items)
        .filter_map(|i| store.item(i).and_downcast::<PackageUpdateObject>())
        .collect();

    store.remove_all();
    for item in items {
        store.append(&item);
    }

    update_statusbar(statusbar, store);
    save_unselected_from_store(store);
}

fn is_core_repo(repository: &str) -> bool {
    return repository.to_ascii_lowercase().contains("core");
}

fn disk_space_warning(store: &ListStore) -> Option<String> {
    let mut net: i64 = 0;
    for i in 0..store.n_items() {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            let data = item.data();
            if data.selected {
                net += data.size;
            }
        }
    }

    let needed = net.max(0);
    let available = available_bytes("/")? as i64;

    if available >= needed + DISK_SAFETY_MARGIN {
        return None;
    }

    let free = glib::format_size(available as u64);
    if needed > 0 {
        return Some(format!(
            "Low disk space. Only {} is free on the system partition, and these updates need about {} more. The install may run out of space and fail.",
            free,
            glib::format_size(needed as u64)
        ));
    }

    return Some(format!(
        "Low disk space. Only {} is free on the system partition. The install may run out of space and fail.",
        free
    ));
}

fn count_core_updates(store: &ListStore) -> (usize, usize) {
    let mut selected = 0;
    let mut total = 0;
    for i in 0..store.n_items() {
        let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() else {
            continue;
        };
        let data = item.data();
        if is_core_repo(&data.repository) {
            total += 1;
            if data.selected {
                selected += 1;
            }
        }
    }
    return (selected, total);
}

fn select_all_official(store: &ListStore) {
    for i in 0..store.n_items() {
        let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() else {
            continue;
        };
        let data = item.data();
        if data.source == PackageSource::Official {
            item.set_selected(true);
        }
    }
}

fn run_install(
    store: &ListStore,
    window: &ApplicationWindow,
    create_snapshot: bool,
    create_snapper: bool,
) {
    if let Err(e) = install_selected_packages_ui(store, window, create_snapshot, create_snapper) {
        log_info!("install failed: {}", e);
        eprintln!("Failed to install packages: {}", e);
    }
}

fn install_selected_packages_ui(
    store: &ListStore,
    window: &ApplicationWindow,
    create_snapshot: bool,
    create_snapper: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut official_packages = Vec::new();
    let mut aur_packages = Vec::new();
    let mut flatpak_packages = Vec::new();
    let mut appimage_packages = Vec::new();
    let n_items = store.n_items();

    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            let data = item.data();
            if data.selected {
                match data.source {
                    PackageSource::Aur => aur_packages.push(data),
                    PackageSource::Flatpak => flatpak_packages.push(data),
                    PackageSource::AppImage => appimage_packages.push(data),
                    PackageSource::Official => official_packages.push(data),
                }
            }
        }
    }

    log_info!(
        "install starting: official={}, aur={}, flatpak={}, appimage={}, snapshot={}, snapper={}",
        official_packages.len(),
        aur_packages.len(),
        flatpak_packages.len(),
        appimage_packages.len(),
        create_snapshot,
        create_snapper
    );
    let pkg_names: Vec<&str> = official_packages
        .iter()
        .chain(aur_packages.iter())
        .chain(flatpak_packages.iter())
        .chain(appimage_packages.iter())
        .map(|p| p.name.as_str())
        .collect();
    if !pkg_names.is_empty() {
        log_info!("packages selected: {}", pkg_names.join(", "));
    }

    if official_packages.is_empty()
        && aur_packages.is_empty()
        && flatpak_packages.is_empty()
        && appimage_packages.is_empty()
    {
        return Ok(());
    }

    if create_snapshot {
        let progress_dialog = create_progress_dialog(
            &window.upcast_ref::<gtk4::Window>(),
            "Creating System Snapshot",
            "Creating Timeshift snapshot and deleting old snapshots...\n\nPlease wait, this may take a few minutes.",
        );

        execute_timeshift_operations_async(
            official_packages.clone(),
            aur_packages.clone(),
            flatpak_packages.clone(),
            appimage_packages.clone(),
            window.clone(),
            progress_dialog,
            create_snapper,
        );

        return Ok(());
    }

    if let Err(e) = navigate_to_terminal_and_install(
        window,
        official_packages,
        aur_packages,
        flatpak_packages,
        appimage_packages,
        create_snapper,
    ) {
        show_error_dialog(
            &window.upcast_ref::<gtk4::Window>(),
            "Installation Error",
            &format!("Failed to start installation: {}", e),
        );
    }
    return Ok(());
}

fn execute_timeshift_operations_async(
    official_packages: Vec<PackageUpdate>,
    aur_packages: Vec<PackageUpdate>,
    flatpak_packages: Vec<PackageUpdate>,
    appimage_packages: Vec<PackageUpdate>,
    window: ApplicationWindow,
    progress_dialog: gtk4::Window,
    create_snapper: bool,
) {
    let (tx, rx) = mpsc::channel();
    let official_packages_clone = official_packages.clone();
    let aur_packages_clone = aur_packages.clone();
    let flatpak_packages_clone = flatpak_packages.clone();
    let appimage_packages_clone = appimage_packages.clone();
    let settings = load_settings();

    thread::spawn(move || match create_timeshift_snapshot(TIMESHIFT_COMMENT) {
        Ok(newest) => match cleanup_timeshift_snapshots(TIMESHIFT_COMMENT, &settings, &newest) {
            Ok(()) => {
                let _ = tx.send(("success", "Package installation starting".to_string()));
            }
            Err(e) => {
                let _ = tx.send(("error", format!("Failed to clean up old snapshots: {}", e)));
            }
        },
        Err(e) => {
            let _ = tx.send(("error", format!("Failed to create system snapshot: {}", e)));
        }
    });

    glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
        Ok(("success", _)) => {
            progress_dialog.close();

            if let Err(e) = navigate_to_terminal_and_install(
                &window,
                official_packages_clone.clone(),
                aur_packages_clone.clone(),
                flatpak_packages_clone.clone(),
                appimage_packages_clone.clone(),
                create_snapper,
            ) {
                show_error_dialog(
                    &window.upcast_ref::<gtk4::Window>(),
                    "Installation Error",
                    &format!("Failed to start installation: {}", e),
                );
            }

            glib::ControlFlow::Break
        }
        Ok(("error", message)) => {
            progress_dialog.close();
            show_error_dialog(
                &window.upcast_ref::<gtk4::Window>(),
                "Timeshift Error",
                &message,
            );
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => {
            progress_dialog.close();
            glib::ControlFlow::Break
        }
        _ => glib::ControlFlow::Continue,
    });
}

fn create_button_content(icon_name: &str, label_text: &str) -> GtkBox {
    let button_box = GtkBox::new(Orientation::Horizontal, 6);
    button_box.set_halign(gtk4::Align::Center);

    let icon = Image::from_icon_name(icon_name);
    let label = gtk4::Label::new(Some(label_text));

    button_box.append(&icon);
    button_box.append(&label);

    return button_box;
}

fn find_terminal_in_box(container: &GtkBox) -> Option<Frame> {
    let mut child = container.first_child();
    while let Some(widget) = child {
        if let Some(frame) = widget.downcast_ref::<Frame>() {
            if frame.label().is_some_and(|label| label == "Terminal") {
                return Some(frame.clone());
            }
        }

        if let Some(child_box) = widget.downcast_ref::<GtkBox>() {
            if let Some(found) = find_terminal_in_box(child_box) {
                return Some(found);
            }
        }
        child = widget.next_sibling();
    }
    return None;
}

fn start_installation_in_terminal(
    terminal: &vte4::Terminal,
    official_packages: Vec<PackageUpdate>,
    aur_packages: Vec<PackageUpdate>,
    flatpak_packages: Vec<PackageUpdate>,
    appimage_packages: Vec<PackageUpdate>,
    create_snapper: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = load_settings();

    let pacman_cmd = if !official_packages.is_empty() {
        let mut parts: Vec<String> = Vec::new();

        let package_groups: Vec<Vec<String>> =
            if settings.separate_repository_groups && !settings.separate_repositories.is_empty() {
                let groups = get_repository_groups()?;

                let mut repo_to_group_id: HashMap<String, String> = HashMap::new();
                for repos in &groups {
                    let group_id = repos.join(",");
                    for repo in repos {
                        repo_to_group_id.insert(repo.clone(), group_id.clone());
                    }
                }

                let mut separated_groups: HashMap<String, Vec<String>> = HashMap::new();
                let mut combined_group: Vec<String> = Vec::new();

                for pkg in &official_packages {
                    if let Some(group_id) = repo_to_group_id.get(&pkg.repository) {
                        if settings.separate_repositories.contains(group_id) {
                            separated_groups
                                .entry(group_id.clone())
                                .or_default()
                                .push(pkg.name.clone());
                        } else {
                            combined_group.push(pkg.name.clone());
                        }
                    } else {
                        combined_group.push(pkg.name.clone());
                    }
                }

                let mut groups: Vec<Vec<String>> = separated_groups.into_values().collect();
                if !combined_group.is_empty() {
                    groups.push(combined_group);
                }
                groups.into_iter().filter(|g| !g.is_empty()).collect()
            } else {
                vec![official_packages.iter().map(|p| p.name.clone()).collect()]
            };

        for mut pkgs in package_groups {
            let pkgs_quoted = pkgs
                .drain(..)
                .map(|p| quote(&p).map(|cow| cow.into_owned()))
                .collect::<Result<Vec<String>, _>>()?
                .join(" ");

            let cmd = format!(
                r#"(expect -c '
set timeout -1
spawn sudo pacman -S {pkgs}
expect {{
    -re {{Proceed with installation\? \[Y/n\]}} {{
        send -- "y\r"
        exp_continue
    }}
    eof
}}
catch wait result
set exit_code [lindex $result 3]
if {{$exit_code eq ""}} {{
    set exit_code 1
}}
exit $exit_code
'
expect_status=$?
while [ -e /var/lib/pacman/db.lck ]; do sleep 0.2; done
exit $expect_status)"#,
                pkgs = pkgs_quoted
            );
            parts.push(cmd);
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" && "))
        }
    } else {
        None
    };

    let aur_cmd = if !aur_packages.is_empty() {
        let parts = install_aur_packages(aur_packages.into_iter().map(|p| p.name).collect())?;
        let line = parts
            .iter()
            .map(|p| quote(&p))
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(|cow| cow.as_ref())
            .collect::<Vec<_>>()
            .join(" ");
        Some(line)
    } else {
        None
    };

    let flatpak_cmd = if !flatpak_packages.is_empty() {
        let refs: Vec<&PackageUpdate> = flatpak_packages.iter().collect();
        build_flatpak_update_command(&refs)
    } else {
        None
    };

    let appimage_cmd = if !appimage_packages.is_empty() {
        let refs: Vec<&PackageUpdate> = appimage_packages.iter().collect();
        let commands = build_appimage_update_commands(&refs);
        if commands.is_empty() {
            None
        } else {
            Some(commands.join(" && "))
        }
    } else {
        None
    };

    let snapper_cmd = if create_snapper {
        Some(build_snapper_snapshot_command())
    } else {
        None
    };

    let parts: Vec<String> = [snapper_cmd, pacman_cmd, aur_cmd, flatpak_cmd, appimage_cmd]
        .into_iter()
        .flatten()
        .collect();

    if parts.is_empty() {
        return Ok(());
    }

    let joined = parts.join(" && ");

    log_info!("spawning install terminal command: {}", joined);
    spawn_terminal(terminal, vec!["bash", "-lc", &joined]);

    return Ok(());
}

fn navigate_to_terminal_and_install(
    window: &ApplicationWindow,
    official_packages: Vec<PackageUpdate>,
    aur_packages: Vec<PackageUpdate>,
    flatpak_packages: Vec<PackageUpdate>,
    appimage_packages: Vec<PackageUpdate>,
    create_snapper: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(main_box) = window.child().and_downcast::<GtkBox>() else {
        return Err("Could not find main box".into());
    };
    let Some(stack) = main_box.first_child().and_downcast::<Stack>() else {
        return Err("Could not find stack".into());
    };
    let Some(terminal_box) = stack.child_by_name("terminal").and_downcast::<GtkBox>() else {
        return Err("Could not find terminal box".into());
    };
    let Some(terminal_frame) = find_terminal_in_box(&terminal_box) else {
        return Err("Could not find terminal frame".into());
    };
    let Some(terminal) = terminal_frame.child().and_downcast::<vte4::Terminal>() else {
        return Err("Could not find terminal widget".into());
    };

    stack.set_visible_child_name("terminal");

    start_installation_in_terminal(
        &terminal,
        official_packages,
        aur_packages,
        flatpak_packages,
        appimage_packages,
        create_snapper,
    )?;

    return Ok(());
}
