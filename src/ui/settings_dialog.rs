use gio::ListStore;
use gtk4::{ApplicationWindow, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    helpers::{
        aur::is_command_available,
        logger::open_logs_folder,
        pacman_repos::get_repository_groups,
        settings::{get_available_aur_helpers, load_settings, save_settings},
        snapper::{is_snap_pac_installed, is_snapper_installed},
        tray_integration::{
            apply_check_schedule, apply_tray_state, has_systemd_user_session, kick_tray,
        },
    },
    log_info,
    models::{
        app_settings::AppSettings, check_schedule::CheckSchedule, snapshot_group::SnapshotGroup,
        snapshot_retention_period::SnapshotRetentionPeriod,
    },
    ui::{
        blacklist_dialog::show_manage_blacklist_dialog, dialogs::show_confirm_dialog,
        favorites_dialog::show_manage_favorites_dialog, package_list::refresh_all_favorite_buttons,
    },
};

pub fn show_settings_dialog(
    parent: &ApplicationWindow,
    settings: &AppSettings,
    favorites_column: Option<gtk4::ColumnViewColumn>,
    package_store: Option<ListStore>,
) {
    install_settings_css();

    let dialog = gtk4::Dialog::builder()
        .title("Settings")
        .transient_for(parent)
        .modal(true)
        .default_width(460)
        .default_height(560)
        .build();

    let content_area = dialog.content_area();
    content_area.set_spacing(0);
    content_area.set_vexpand(true);

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.set_vexpand(true);

    let switcher = gtk4::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk4::Align::Fill);
    switcher.set_hexpand(true);
    switcher.set_margin_start(16);
    switcher.set_margin_end(16);
    switcher.set_margin_top(8);
    switcher.set_margin_bottom(8);

    let general_container = build_tab_container();
    let snapshot_group = create_snapshot_group(settings, &general_container);
    let remember_unselected_check = create_remember_unselected_group(settings, &general_container);
    let (
        system_tray_check,
        always_visible_check,
        only_favorites_check,
        menu_only_favorites_check,
        notify_check,
        check_schedule_combo,
        skip_metered_check,
        skip_battery_check,
    ) = create_system_tray_group(settings, &general_container);
    let log_retention_spin = create_logs_group(settings, &general_container);
    stack.add_titled(&wrap_tab(&general_container), Some("general"), "General");

    let packages_container = build_tab_container();
    let (aur_enable_check, aur_combo, aur_devel_check) =
        create_aur_group(settings, &packages_container);
    let flatpak_enable_check = create_flatpak_group(settings, &packages_container);
    stack.add_titled(&wrap_tab(&packages_container), Some("packages"), "Packages");

    let pacman_container = build_tab_container();
    let (separate_repo_check, repo_checkboxes) = create_packages_group(settings, &pacman_container);
    let (keep_old_spin, keep_uninstalled_spin) = create_cache_group(settings, &pacman_container);
    create_blacklist_group(&pacman_container, parent);
    stack.add_titled(&wrap_tab(&pacman_container), Some("pacman"), "Pacman");

    let interface_container = build_tab_container();
    let show_desc_check = create_show_descriptions_group(settings, &interface_container);
    let post_update_check = create_post_update_group(settings, &interface_container);
    let (fav_enable_check, fav_show_col_check, manage_btn, mode_btn) =
        create_favorites_group(settings, &interface_container, parent);
    stack.add_titled(
        &wrap_tab(&interface_container),
        Some("interface"),
        "Interface",
    );

    content_area.append(&switcher);
    content_area.append(&stack);

    let switcher_clone = switcher.clone();
    glib::idle_add_local_once(move || {
        pad_switcher_buttons(&switcher_clone);
    });

    let save_all = {
        let aur_enable_check = aur_enable_check.clone();
        let aur_combo = aur_combo.clone();
        let aur_devel_check = aur_devel_check.clone();
        let snapshot_enable_check = snapshot_group.enable_check.clone();
        let snapshot_provider_combo = snapshot_group.provider_combo.clone();
        let retention_count_spin = snapshot_group.retention_count_spin.clone();
        let retention_period_combo = snapshot_group.retention_period_combo.clone();
        let fav_enable_check = fav_enable_check.clone();
        let fav_show_col_check = fav_show_col_check.clone();
        let separate_repo_check = separate_repo_check.clone();
        let repo_checkboxes = repo_checkboxes.clone();
        let remember_unselected_check = remember_unselected_check.clone();
        let post_update_check = post_update_check.clone();
        let flatpak_enable_check = flatpak_enable_check.clone();
        let keep_old_spin = keep_old_spin.clone();
        let keep_uninstalled_spin = keep_uninstalled_spin.clone();
        let system_tray_check = system_tray_check.clone();
        let always_visible_check = always_visible_check.clone();
        let only_favorites_check = only_favorites_check.clone();
        let menu_only_favorites_check = menu_only_favorites_check.clone();
        let notify_check = notify_check.clone();
        let check_schedule_combo = check_schedule_combo.clone();
        let skip_metered_check = skip_metered_check.clone();
        let skip_battery_check = skip_battery_check.clone();
        let show_desc_check = show_desc_check.clone();
        let log_retention_spin = log_retention_spin.clone();

        Rc::new(move || {
            let mut new_settings = load_settings();

            new_settings.enable_aur_support = aur_enable_check.is_active();

            if let Some(active_id) = aur_combo.active_id() {
                new_settings.preferred_aur_helper = if active_id == "auto" {
                    None
                } else {
                    Some(active_id.to_string())
                };
            }

            new_settings.enable_devel_aur = aur_devel_check.is_active();

            let snapshots_enabled = snapshot_enable_check.is_active();
            let provider = snapshot_provider_combo.active_id();
            let provider_str = provider.as_deref();
            new_settings.create_timeshift_snapshot =
                snapshots_enabled && provider_str == Some("timeshift");
            new_settings.create_snapper_snapshot =
                snapshots_enabled && provider_str == Some("snapper");
            new_settings.snapshot_retention_count = retention_count_spin.value() as u32;

            if let Some(active_id) = retention_period_combo.active_id() {
                new_settings.snapshot_retention_period = match active_id.as_str() {
                    "day" => SnapshotRetentionPeriod::Day,
                    "week" => SnapshotRetentionPeriod::Week,
                    "month" => SnapshotRetentionPeriod::Month,
                    "year" => SnapshotRetentionPeriod::Year,
                    _ => SnapshotRetentionPeriod::Forever,
                };
            }

            new_settings.enable_favorites = fav_enable_check.is_active();
            new_settings.show_favorites_column = fav_show_col_check.is_active();

            new_settings.separate_repository_groups = separate_repo_check.is_active();

            let mut selected_repos = Vec::new();
            for (repo_id, checkbox) in repo_checkboxes.borrow().iter() {
                if checkbox.is_active() {
                    selected_repos.push(repo_id.clone());
                }
            }
            new_settings.separate_repositories = selected_repos;

            new_settings.remember_unselected_packages = remember_unselected_check.is_active();
            new_settings.run_post_update_checks = post_update_check.is_active();
            new_settings.enable_flatpak_support = flatpak_enable_check.is_active();
            new_settings.keep_old_packages = keep_old_spin.value() as u32;
            new_settings.keep_uninstalled_packages = keep_uninstalled_spin.value() as u32;
            new_settings.enable_system_tray = system_tray_check.is_active();
            new_settings.tray_always_visible =
                system_tray_check.is_active() && always_visible_check.is_active();
            new_settings.tray_only_favorites =
                system_tray_check.is_active() && only_favorites_check.is_active();
            new_settings.tray_menu_only_favorites =
                system_tray_check.is_active() && menu_only_favorites_check.is_active();
            new_settings.show_update_notifications =
                system_tray_check.is_active() && notify_check.is_active();
            if let Some(active_id) = check_schedule_combo.active_id() {
                new_settings.check_schedule = CheckSchedule::from_id(&active_id);
            }
            new_settings.skip_check_on_metered =
                system_tray_check.is_active() && skip_metered_check.is_active();
            new_settings.skip_check_on_battery =
                system_tray_check.is_active() && skip_battery_check.is_active();
            new_settings.show_package_descriptions = show_desc_check.is_active();
            new_settings.log_retention_days = log_retention_spin.value() as u32;

            if let Err(e) = save_settings(&new_settings) {
                log_info!("failed to save settings: {}", e);
                eprintln!("Failed to save settings: {}", e);
            } else {
                log_info!("settings saved");
            }
        })
    };

    let aur_combo_weak = aur_combo.clone();
    let aur_devel_check_weak = aur_devel_check.clone();
    let save_all_clone = save_all.clone();
    aur_enable_check.connect_toggled(move |check| {
        let is_active = check.is_active();
        aur_combo_weak.set_sensitive(is_active);
        aur_devel_check_weak.set_sensitive(is_active);
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    aur_combo.connect_changed(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    aur_devel_check.connect_toggled(move |_| {
        save_all_clone();
    });

    wire_snapshot_group_signals(&snapshot_group, save_all.clone());

    let fav_show_col_check_weak = fav_show_col_check.downgrade();
    let manage_btn_weak = manage_btn.downgrade();
    let mode_btn_weak = mode_btn.downgrade();
    let favorites_column2 = favorites_column.clone();
    let save_all_clone = save_all.clone();
    fav_enable_check.connect_toggled(move |check| {
        let is_enabled = check.is_active();
        if let Some(col) = &favorites_column {
            let show_col = fav_show_col_check_weak
                .upgrade()
                .map(|c| c.is_active())
                .unwrap_or(false);
            col.set_visible(is_enabled && show_col);
        }
        if let Some(c) = fav_show_col_check_weak.upgrade() {
            c.set_sensitive(is_enabled);
        }
        if let Some(btn) = manage_btn_weak.upgrade() {
            btn.set_sensitive(is_enabled);
        }
        if let Some(btn) = mode_btn_weak.upgrade() {
            btn.set_sensitive(is_enabled);
        }
        save_all_clone();
    });

    let fav_enable_check_weak = fav_enable_check.downgrade();
    let save_all_clone = save_all.clone();
    fav_show_col_check.connect_toggled(move |check| {
        if let Some(col) = &favorites_column2 {
            let is_enabled = fav_enable_check_weak
                .upgrade()
                .map(|c| c.is_active())
                .unwrap_or(false);
            col.set_visible(is_enabled && check.is_active());
        }
        save_all_clone();
    });

    let repo_checkboxes_weak = repo_checkboxes.clone();
    let save_all_clone = save_all.clone();
    separate_repo_check.connect_toggled(move |check| {
        let is_active = check.is_active();
        for (_, checkbox) in repo_checkboxes_weak.borrow().iter() {
            checkbox.set_sensitive(is_active);
        }
        save_all_clone();
    });

    for (_, checkbox) in repo_checkboxes.borrow().iter() {
        let save_all_clone = save_all.clone();
        checkbox.connect_toggled(move |_| {
            save_all_clone();
        });
    }

    let save_all_clone = save_all.clone();
    remember_unselected_check.connect_toggled(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    post_update_check.connect_toggled(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    flatpak_enable_check.connect_toggled(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    keep_old_spin.connect_value_changed(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    keep_uninstalled_spin.connect_value_changed(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    let notify_check_weak = notify_check.clone();
    let always_visible_check_weak = always_visible_check.clone();
    let only_favorites_check_weak = only_favorites_check.clone();
    let menu_only_favorites_check_weak = menu_only_favorites_check.clone();
    let check_schedule_combo_weak = check_schedule_combo.clone();
    let skip_metered_check_weak = skip_metered_check.clone();
    let skip_battery_check_weak = skip_battery_check.clone();
    system_tray_check.connect_toggled(move |check| {
        notify_check_weak.set_sensitive(check.is_active());
        always_visible_check_weak.set_sensitive(check.is_active());
        only_favorites_check_weak.set_sensitive(check.is_active());
        menu_only_favorites_check_weak.set_sensitive(check.is_active());
        check_schedule_combo_weak.set_sensitive(check.is_active());
        skip_metered_check_weak.set_sensitive(check.is_active());
        skip_battery_check_weak.set_sensitive(check.is_active());
        save_all_clone();
        if check.is_active() {
            let schedule = check_schedule_combo_weak
                .active_id()
                .map(|id| CheckSchedule::from_id(&id))
                .unwrap_or_default();
            apply_check_schedule(schedule);
        }
        apply_tray_state(check.is_active());
    });

    let save_all_clone = save_all.clone();
    notify_check.connect_toggled(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    let only_favorites_for_excl = only_favorites_check.clone();
    always_visible_check.connect_toggled(move |btn| {
        if btn.is_active() && only_favorites_for_excl.is_active() {
            only_favorites_for_excl.set_active(false);
        }
        save_all_clone();
        kick_tray();
    });

    let save_all_clone = save_all.clone();
    let always_visible_for_excl = always_visible_check.clone();
    only_favorites_check.connect_toggled(move |btn| {
        if btn.is_active() && always_visible_for_excl.is_active() {
            always_visible_for_excl.set_active(false);
        }
        save_all_clone();
        kick_tray();
    });

    let save_all_clone = save_all.clone();
    menu_only_favorites_check.connect_toggled(move |_| {
        save_all_clone();
        kick_tray();
    });

    let save_all_clone = save_all.clone();
    skip_metered_check.connect_toggled(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    skip_battery_check.connect_toggled(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    check_schedule_combo.connect_changed(move |combo| {
        save_all_clone();
        let schedule = combo
            .active_id()
            .map(|id| CheckSchedule::from_id(&id))
            .unwrap_or_default();
        apply_check_schedule(schedule);
    });

    let save_all_clone = save_all.clone();
    log_retention_spin.connect_value_changed(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    let package_store_for_desc = package_store.clone();
    show_desc_check.connect_toggled(move |_| {
        save_all_clone();
        if let Some(store) = &package_store_for_desc {
            let n = store.n_items();
            if n > 0 {
                store.items_changed(0, n, n);
            }
        }
    });

    dialog.present();
}

fn create_aur_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::CheckButton, gtk4::ComboBoxText, gtk4::CheckButton) {
    let aur_section = create_preference_group(
        "AUR Package Manager",
        "Enable support for installing packages from the Arch User Repository (AUR).",
    );

    let aur_enable_check = gtk4::CheckButton::with_label("Enable AUR support");
    aur_enable_check.add_css_class("settings-check");
    aur_enable_check.set_active(settings.enable_aur_support);
    aur_section.append(&aur_enable_check);

    let available_helpers = get_available_aur_helpers();
    let aur_combo = gtk4::ComboBoxText::new();
    aur_combo.add_css_class("settings-combo");
    aur_combo.set_margin_top(8);

    aur_combo.append(Some("auto"), "Auto-detect (recommended)");
    for helper in &available_helpers {
        aur_combo.append(Some(helper), helper);
    }

    if let Some(preferred) = &settings.preferred_aur_helper {
        aur_combo.set_active_id(Some(preferred));
    } else {
        aur_combo.set_active_id(Some("auto"));
    }

    aur_combo.set_sensitive(settings.enable_aur_support);

    aur_section.append(&aur_combo);

    let devel_check = gtk4::CheckButton::with_label("Check development packages (devel mode)");
    devel_check.add_css_class("settings-check");
    devel_check.set_active(settings.enable_devel_aur);
    devel_check.set_margin_top(12);
    devel_check.set_sensitive(settings.enable_aur_support);
    devel_check.set_tooltip_text(Some(
        "Also check git, svn, and bzr packages for new commits, not just version bumps.",
    ));
    aur_section.append(&devel_check);

    main_container.append(&aur_section);

    return (aur_enable_check, aur_combo, devel_check);
}

fn create_show_descriptions_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Package List Display",
        "Show a short description under each package name in the update list.",
    );

    let check = gtk4::CheckButton::with_label("Show package descriptions");
    check.add_css_class("settings-check");
    check.set_active(settings.show_package_descriptions);
    section.append(&check);

    main_container.append(&section);

    return check;
}

fn create_system_tray_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (
    gtk4::CheckButton,
    gtk4::CheckButton,
    gtk4::CheckButton,
    gtk4::CheckButton,
    gtk4::CheckButton,
    gtk4::ComboBoxText,
    gtk4::CheckButton,
    gtk4::CheckButton,
) {
    let systemd_available = has_systemd_user_session();

    let section = create_preference_group(
        "System Tray",
        "Show a system tray icon that displays the number of pending updates. Runs as a user-level systemd service that starts at login.",
    );

    let check = gtk4::CheckButton::with_label("Show system tray icon");
    check.add_css_class("settings-check");
    check.set_active(settings.enable_system_tray && systemd_available);
    check.set_sensitive(systemd_available);
    section.append(&check);

    let always_visible_check =
        gtk4::CheckButton::with_label("Always show tray icon (even when system is up to date)");
    always_visible_check.add_css_class("settings-check");
    always_visible_check.set_active(settings.tray_always_visible && systemd_available);
    always_visible_check.set_sensitive(systemd_available && settings.enable_system_tray);
    always_visible_check.set_margin_top(8);
    always_visible_check.set_margin_start(24);
    section.append(&always_visible_check);

    let only_favorites_check =
        gtk4::CheckButton::with_label("Show tray icon only when a favorite package has an update");
    only_favorites_check.add_css_class("settings-check");
    only_favorites_check.set_active(settings.tray_only_favorites && systemd_available);
    only_favorites_check.set_sensitive(systemd_available && settings.enable_system_tray);
    only_favorites_check.set_margin_top(8);
    only_favorites_check.set_margin_start(24);
    section.append(&only_favorites_check);

    let menu_only_favorites_check =
        gtk4::CheckButton::with_label("Show only favorite packages in the tray menu");
    menu_only_favorites_check.add_css_class("settings-check");
    menu_only_favorites_check.set_active(settings.tray_menu_only_favorites && systemd_available);
    menu_only_favorites_check.set_sensitive(systemd_available && settings.enable_system_tray);
    menu_only_favorites_check.set_margin_top(8);
    menu_only_favorites_check.set_margin_start(24);
    section.append(&menu_only_favorites_check);

    let interval_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    interval_box.set_margin_top(8);
    interval_box.set_margin_start(24);
    interval_box.set_hexpand(true);

    let interval_label = gtk4::Label::new(Some("Check for updates"));
    interval_label.set_halign(gtk4::Align::Start);
    interval_label.set_hexpand(true);
    interval_box.append(&interval_label);

    let check_schedule_combo = gtk4::ComboBoxText::new();
    check_schedule_combo.add_css_class("settings-combo");
    for schedule in CheckSchedule::all() {
        check_schedule_combo.append(Some(schedule.id()), schedule.label());
    }
    check_schedule_combo.set_active_id(Some(settings.check_schedule.id()));
    check_schedule_combo.set_halign(gtk4::Align::End);
    check_schedule_combo.set_sensitive(systemd_available && settings.enable_system_tray);
    interval_box.append(&check_schedule_combo);

    interval_label.set_sensitive(systemd_available && settings.enable_system_tray);
    section.append(&interval_box);

    let notify_check =
        gtk4::CheckButton::with_label("Show desktop notification when updates are available");
    notify_check.add_css_class("settings-check");
    notify_check.set_active(settings.show_update_notifications && systemd_available);
    notify_check.set_sensitive(systemd_available && settings.enable_system_tray);
    notify_check.set_margin_top(8);
    notify_check.set_margin_start(24);
    section.append(&notify_check);

    let skip_metered_check =
        gtk4::CheckButton::with_label("Skip check on metered network connections");
    skip_metered_check.add_css_class("settings-check");
    skip_metered_check.set_active(settings.skip_check_on_metered && systemd_available);
    skip_metered_check.set_sensitive(systemd_available && settings.enable_system_tray);
    skip_metered_check.set_margin_top(8);
    skip_metered_check.set_margin_start(24);
    section.append(&skip_metered_check);

    let skip_battery_check =
        gtk4::CheckButton::with_label("Skip check when running on battery power");
    skip_battery_check.add_css_class("settings-check");
    skip_battery_check.set_active(settings.skip_check_on_battery && systemd_available);
    skip_battery_check.set_sensitive(systemd_available && settings.enable_system_tray);
    skip_battery_check.set_margin_top(8);
    skip_battery_check.set_margin_start(24);
    section.append(&skip_battery_check);

    if !systemd_available {
        let warning = gtk4::Label::new(Some(
            "A systemd user session is required to use the tray. This system does not appear to have one available.",
        ));
        warning.set_wrap(true);
        warning.set_xalign(0.0);
        warning.set_margin_top(8);
        warning.add_css_class("dim-label");
        warning.add_css_class("caption");
        section.append(&warning);
    }

    main_container.append(&section);

    return (
        check,
        always_visible_check,
        only_favorites_check,
        menu_only_favorites_check,
        notify_check,
        check_schedule_combo,
        skip_metered_check,
        skip_battery_check,
    );
}

fn create_snapshot_group(settings: &AppSettings, main_container: &gtk4::Box) -> SnapshotGroup {
    let has_timeshift = is_command_available("timeshift");
    let has_snapper = is_snapper_installed();
    let snap_pac_installed = is_snap_pac_installed();

    let section = create_preference_group(
        "System Snapshots",
        "Automatically create a system snapshot before installing updates for easy rollback if needed.",
    );

    let enable_check = gtk4::CheckButton::with_label("Create a system snapshot before the update");
    enable_check.add_css_class("settings-check");

    let initial_enabled = (settings.create_timeshift_snapshot && has_timeshift)
        || (settings.create_snapper_snapshot && has_snapper);
    enable_check.set_active(initial_enabled);
    section.append(&enable_check);

    if !has_timeshift && !has_snapper {
        enable_check.set_sensitive(false);
        let info = gtk4::Label::new(Some(
            "Install timeshift or snapper to enable system snapshots.",
        ));
        info.set_wrap(true);
        info.set_xalign(0.0);
        info.set_margin_top(8);
        info.add_css_class("dim-label");
        info.add_css_class("caption");
        section.append(&info);
    }

    let provider_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    provider_box.set_margin_top(12);
    provider_box.set_hexpand(true);

    let provider_label = gtk4::Label::new(Some("Snapshot provider"));
    provider_label.set_halign(gtk4::Align::Start);
    provider_label.set_hexpand(true);
    provider_box.append(&provider_label);

    let provider_combo = gtk4::ComboBoxText::new();
    provider_combo.add_css_class("settings-combo");
    if has_timeshift {
        provider_combo.append(Some("timeshift"), "Timeshift");
    }
    if has_snapper {
        provider_combo.append(Some("snapper"), "Snapper");
    }

    let active_provider = if settings.create_snapper_snapshot && has_snapper {
        "snapper"
    } else if settings.create_timeshift_snapshot && has_timeshift {
        "timeshift"
    } else if has_timeshift {
        "timeshift"
    } else if has_snapper {
        "snapper"
    } else {
        ""
    };
    if !active_provider.is_empty() {
        provider_combo.set_active_id(Some(active_provider));
    }
    provider_combo.set_halign(gtk4::Align::End);
    provider_box.append(&provider_combo);
    section.append(&provider_box);

    let retention_count_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    retention_count_box.set_margin_top(12);
    retention_count_box.set_hexpand(true);

    let retention_count_label = gtk4::Label::new(Some("Number of snapshots to keep"));
    retention_count_label.set_halign(gtk4::Align::Start);
    retention_count_label.set_hexpand(true);
    retention_count_box.append(&retention_count_label);

    let retention_count_spin = gtk4::SpinButton::with_range(1.0, 10.0, 1.0);
    retention_count_spin.set_value(settings.snapshot_retention_count as f64);
    retention_count_spin.add_css_class("settings-spin");
    retention_count_spin.set_halign(gtk4::Align::End);
    retention_count_box.append(&retention_count_spin);

    section.append(&retention_count_box);

    let retention_period_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    retention_period_box.set_margin_top(8);
    retention_period_box.set_hexpand(true);

    let retention_period_label = gtk4::Label::new(Some("Keep snapshots for"));
    retention_period_label.set_halign(gtk4::Align::Start);
    retention_period_label.set_hexpand(true);
    retention_period_box.append(&retention_period_label);

    let retention_period_combo = gtk4::ComboBoxText::new();
    retention_period_combo.add_css_class("settings-combo");
    retention_period_combo.append(Some("forever"), "Forever");
    retention_period_combo.append(Some("day"), "1 Day");
    retention_period_combo.append(Some("week"), "1 Week");
    retention_period_combo.append(Some("month"), "1 Month");
    retention_period_combo.append(Some("year"), "1 Year");

    let active_id = match settings.snapshot_retention_period {
        SnapshotRetentionPeriod::Forever => "forever",
        SnapshotRetentionPeriod::Day => "day",
        SnapshotRetentionPeriod::Week => "week",
        SnapshotRetentionPeriod::Month => "month",
        SnapshotRetentionPeriod::Year => "year",
    };
    retention_period_combo.set_active_id(Some(active_id));
    retention_period_combo.set_halign(gtk4::Align::End);
    retention_period_box.append(&retention_period_combo);

    section.append(&retention_period_box);

    let deletion_info_label =
        gtk4::Label::new(Some("Old snapshots are only deleted when updating."));
    deletion_info_label.set_wrap(true);
    deletion_info_label.set_xalign(0.0);
    deletion_info_label.set_margin_top(8);
    deletion_info_label.add_css_class("dim-label");
    deletion_info_label.add_css_class("caption");
    section.append(&deletion_info_label);

    let snap_pac_info = gtk4::Label::new(Some(
        "The snap-pac package is installed, so Snapper already creates a snapshot automatically before each pacman transaction. No extra action is needed.",
    ));
    snap_pac_info.set_wrap(true);
    snap_pac_info.set_xalign(0.0);
    snap_pac_info.set_margin_top(8);
    snap_pac_info.add_css_class("dim-label");
    snap_pac_info.add_css_class("caption");
    snap_pac_info.set_visible(false);
    section.append(&snap_pac_info);

    main_container.append(&section);

    let selected_timeshift = provider_combo
        .active_id()
        .map(|id| id == "timeshift")
        .unwrap_or(false);
    let selected_snapper = provider_combo
        .active_id()
        .map(|id| id == "snapper")
        .unwrap_or(false);

    provider_box.set_sensitive(initial_enabled);
    retention_count_box.set_sensitive(initial_enabled && selected_timeshift);
    retention_period_box.set_sensitive(initial_enabled && selected_timeshift);
    retention_count_box.set_visible(selected_timeshift);
    retention_period_box.set_visible(selected_timeshift);
    deletion_info_label.set_visible(selected_timeshift);
    snap_pac_info.set_visible(initial_enabled && selected_snapper && snap_pac_installed);

    return SnapshotGroup {
        enable_check,
        provider_combo,
        retention_count_spin,
        retention_period_combo,
        retention_count_box,
        retention_period_box,
        deletion_info_label,
        snap_pac_info,
        has_timeshift,
        has_snapper,
        snap_pac_installed,
    };
}

fn wire_snapshot_group_signals(group: &SnapshotGroup, save_all: Rc<dyn Fn()>) {
    let provider_combo_w = group.provider_combo.clone();
    let retention_count_box_w = group.retention_count_box.clone();
    let retention_period_box_w = group.retention_period_box.clone();
    let deletion_info_label_w = group.deletion_info_label.clone();
    let snap_pac_info_w = group.snap_pac_info.clone();
    let snap_pac_installed = group.snap_pac_installed;
    let save_all_clone = save_all.clone();
    group.enable_check.connect_toggled(move |check| {
        let enabled = check.is_active();
        provider_combo_w.set_sensitive(enabled);
        let is_timeshift = provider_combo_w
            .active_id()
            .map(|id| id == "timeshift")
            .unwrap_or(false);
        let is_snapper = provider_combo_w
            .active_id()
            .map(|id| id == "snapper")
            .unwrap_or(false);
        retention_count_box_w.set_sensitive(enabled && is_timeshift);
        retention_period_box_w.set_sensitive(enabled && is_timeshift);
        retention_count_box_w.set_visible(is_timeshift);
        retention_period_box_w.set_visible(is_timeshift);
        deletion_info_label_w.set_visible(is_timeshift);
        snap_pac_info_w.set_visible(enabled && is_snapper && snap_pac_installed);
        save_all_clone();
    });

    let enable_check_w = group.enable_check.clone();
    let retention_count_box_w = group.retention_count_box.clone();
    let retention_period_box_w = group.retention_period_box.clone();
    let deletion_info_label_w = group.deletion_info_label.clone();
    let snap_pac_info_w = group.snap_pac_info.clone();
    let save_all_clone = save_all.clone();
    group.provider_combo.connect_changed(move |combo| {
        let enabled = enable_check_w.is_active();
        let is_timeshift = combo
            .active_id()
            .map(|id| id == "timeshift")
            .unwrap_or(false);
        let is_snapper = combo.active_id().map(|id| id == "snapper").unwrap_or(false);
        retention_count_box_w.set_sensitive(enabled && is_timeshift);
        retention_period_box_w.set_sensitive(enabled && is_timeshift);
        retention_count_box_w.set_visible(is_timeshift);
        retention_period_box_w.set_visible(is_timeshift);
        deletion_info_label_w.set_visible(is_timeshift);
        snap_pac_info_w.set_visible(enabled && is_snapper && snap_pac_installed);
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    group
        .retention_count_spin
        .connect_value_changed(move |_| save_all_clone());

    let save_all_clone = save_all.clone();
    group
        .retention_period_combo
        .connect_changed(move |_| save_all_clone());
}

fn create_favorites_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
    parent: &ApplicationWindow,
) -> (
    gtk4::CheckButton,
    gtk4::CheckButton,
    gtk4::Button,
    gtk4::Button,
) {
    let section = create_preference_group(
        "Favorite Packages",
        "Mark packages as favorites to show them at the top of the package list.",
    );

    let enable_check = gtk4::CheckButton::with_label("Enable favorite packages");
    enable_check.add_css_class("settings-check");
    enable_check.set_active(settings.enable_favorites);
    section.append(&enable_check);

    let show_col_check = gtk4::CheckButton::with_label("Show favorites column in package list");
    show_col_check.add_css_class("settings-check");
    show_col_check.set_active(settings.show_favorites_column);
    show_col_check.set_sensitive(settings.enable_favorites);
    section.append(&show_col_check);

    let manage_btn = build_padded_button("Manage Favorite Packages");
    manage_btn.set_sensitive(settings.enable_favorites);
    manage_btn.set_halign(gtk4::Align::Start);
    let parent_clone = parent.clone();
    manage_btn.connect_clicked(move |_| {
        show_manage_favorites_dialog(parent_clone.upcast_ref::<gtk4::Window>());
    });
    section.append(&manage_btn);

    let mode_btn_label = if settings.favorites_exclusion_mode {
        "Switch to Inclusion Mode"
    } else {
        "Switch to Exclusion Mode"
    };
    let mode_btn = build_padded_button(mode_btn_label);
    mode_btn.set_sensitive(settings.enable_favorites);
    mode_btn.set_halign(gtk4::Align::Start);
    update_mode_button_tooltip(&mode_btn, settings.favorites_exclusion_mode);
    let parent_for_mode = parent.clone();
    mode_btn.connect_clicked(move |btn| {
        let current = load_settings();
        let switching_to_exclusion = !current.favorites_exclusion_mode;
        let (title, message, accept_label) = if switching_to_exclusion {
            (
                "Switch to exclusion mode?",
                "Every installed package becomes a favorite by default. Your current favorites list will be cleared and instead used to track packages you exclude from favorites.",
                "Switch",
            )
        } else {
            (
                "Switch to inclusion mode?",
                "Your current exclusion list will be cleared. After this, no package is a favorite until you mark it.",
                "Switch",
            )
        };
        let dialog = show_confirm_dialog(&parent_for_mode, title, message, accept_label);
        let btn_for_response = btn.clone();
        dialog.connect_response(move |dialog, response| {
            if response == gtk4::ResponseType::Accept {
                let mut s = load_settings();
                s.favorites_exclusion_mode = switching_to_exclusion;
                s.favorite_packages.clear();
                if let Err(e) = save_settings(&s) {
                    eprintln!("Failed to save favorites mode: {}", e);
                } else {
                    refresh_all_favorite_buttons(switching_to_exclusion);
                    btn_for_response.set_label(if switching_to_exclusion {
                        "Switch to Inclusion Mode"
                    } else {
                        "Switch to Exclusion Mode"
                    });
                    update_mode_button_tooltip(&btn_for_response, switching_to_exclusion);
                    kick_tray();
                }
            }
            dialog.close();
        });
    });
    section.append(&mode_btn);

    main_container.append(&section);

    return (enable_check, show_col_check, manage_btn, mode_btn);
}

fn update_mode_button_tooltip(button: &gtk4::Button, exclusion_mode: bool) {
    let tooltip = if exclusion_mode {
        "You are in exclusion mode. Click to switch back to inclusion mode. The current exclusion list will be cleared."
    } else {
        "Switch to exclusion mode. Every installed package will become a favorite by default. You can then uncheck the ones you do not want."
    };
    button.set_tooltip_text(Some(tooltip));
}

fn create_packages_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (
    gtk4::CheckButton,
    Rc<RefCell<Vec<(String, gtk4::CheckButton)>>>,
) {
    let section = create_preference_group(
        "Separate Repository Groups",
        "Separate packages from different repository groups during updates based on the servers they come from. This way packages from the official Arch Linux repositories will be handled separately from those from third-party repositories and if the servers are down there will still be a partial update.",
    );

    let enable_check =
        gtk4::CheckButton::with_label("Enable separate repository group installation");
    enable_check.add_css_class("settings-check");
    enable_check.set_active(settings.separate_repository_groups);
    section.append(&enable_check);

    let repo_checkboxes: Rc<RefCell<Vec<(String, gtk4::CheckButton)>>> =
        Rc::new(RefCell::new(Vec::new()));

    match get_repository_groups() {
        Ok(groups) => {
            if groups.len() > 1 {
                let repos_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                repos_box.set_margin_top(12);
                repos_box.set_margin_start(24);

                for repos in groups {
                    let label_text = repos.join(", ");
                    let repo_id = repos.join(",");

                    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

                    let checkbox = gtk4::CheckButton::new();
                    checkbox.add_css_class("settings-check");
                    checkbox.set_active(settings.separate_repositories.contains(&repo_id));
                    checkbox.set_sensitive(settings.separate_repository_groups);
                    row.append(&checkbox);

                    let label = gtk4::Label::new(Some(&label_text));
                    label.set_wrap(true);
                    label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);

                    let click = gtk4::GestureClick::new();
                    let checkbox_weak = checkbox.downgrade();
                    click.connect_released(move |_, _, _, _| {
                        if let Some(cb) = checkbox_weak.upgrade() {
                            if cb.is_sensitive() {
                                cb.set_active(!cb.is_active());
                            }
                        }
                    });
                    label.add_controller(click);

                    row.append(&label);

                    repos_box.append(&row);
                    repo_checkboxes.borrow_mut().push((repo_id, checkbox));
                }

                section.append(&repos_box);
            } else {
                let info_label = gtk4::Label::new(Some(
                    "Only one repository group detected. No separation needed.",
                ));
                info_label.set_wrap(true);
                info_label.set_xalign(0.0);
                info_label.set_margin_top(8);
                info_label.add_css_class("dim-label");
                section.append(&info_label);
            }
        }
        Err(e) => {
            eprintln!("Failed to get repository groups: {}", e);
            let error_label = gtk4::Label::new(Some("Failed to detect repository groups."));
            error_label.set_wrap(true);
            error_label.set_xalign(0.0);
            error_label.set_margin_top(8);
            error_label.add_css_class("dim-label");
            section.append(&error_label);
        }
    }

    main_container.append(&section);

    return (enable_check, repo_checkboxes);
}

fn create_remember_unselected_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Remember Package Selection",
        "Remember which packages were unselected between sessions.",
    );

    let check = gtk4::CheckButton::with_label("Remember unselected packages");
    check.add_css_class("settings-check");
    check.set_active(settings.remember_unselected_packages);

    section.append(&check);
    main_container.append(&section);

    return check;
}

fn create_post_update_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Post-Update Checks",
        "After installing updates, open a checks page that helps with orphan packages, cache cleanup, services that need a restart, and more.",
    );

    let check = gtk4::CheckButton::with_label("Run checks after install");
    check.add_css_class("settings-check");
    check.set_active(settings.run_post_update_checks);

    section.append(&check);
    main_container.append(&section);

    return check;
}

fn create_flatpak_group(settings: &AppSettings, main_container: &gtk4::Box) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Flatpak Packages",
        "Show updates for Flatpak applications next to system packages.",
    );

    let flatpak_present = is_flatpak_installed();

    let check = gtk4::CheckButton::with_label("Enable Flatpak support");
    check.add_css_class("settings-check");
    check.set_active(settings.enable_flatpak_support && flatpak_present);
    check.set_sensitive(flatpak_present);

    section.append(&check);

    if !flatpak_present {
        let warning = gtk4::Label::new(Some(
            "The flatpak command is not installed on this system. Install the flatpak package to use this feature.",
        ));
        warning.set_wrap(true);
        warning.set_xalign(0.0);
        warning.set_margin_top(8);
        warning.add_css_class("dim-label");
        warning.add_css_class("caption");
        section.append(&warning);
    }

    main_container.append(&section);

    return check;
}

fn create_cache_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::SpinButton, gtk4::SpinButton) {
    let section = create_preference_group(
        "Pacman Cache",
        "Choose how many old and uninstalled package versions to keep in the pacman cache. The cleanup runs from the post-update checks page.",
    );

    let old_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    old_box.set_hexpand(true);

    let old_label = gtk4::Label::new(Some("Versions of installed packages to keep"));
    old_label.set_halign(gtk4::Align::Start);
    old_label.set_hexpand(true);
    old_box.append(&old_label);

    let old_spin = gtk4::SpinButton::with_range(0.0, 50.0, 1.0);
    old_spin.set_value(settings.keep_old_packages as f64);
    old_spin.add_css_class("settings-spin");
    old_spin.set_halign(gtk4::Align::End);
    old_box.append(&old_spin);

    section.append(&old_box);

    let uninst_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    uninst_box.set_margin_top(8);
    uninst_box.set_hexpand(true);

    let uninst_label = gtk4::Label::new(Some("Versions of uninstalled packages to keep"));
    uninst_label.set_halign(gtk4::Align::Start);
    uninst_label.set_hexpand(true);
    uninst_box.append(&uninst_label);

    let uninst_spin = gtk4::SpinButton::with_range(0.0, 50.0, 1.0);
    uninst_spin.set_value(settings.keep_uninstalled_packages as f64);
    uninst_spin.add_css_class("settings-spin");
    uninst_spin.set_halign(gtk4::Align::End);
    uninst_box.append(&uninst_spin);

    section.append(&uninst_box);
    main_container.append(&section);

    return (old_spin, uninst_spin);
}

fn create_blacklist_group(main_container: &gtk4::Box, parent: &ApplicationWindow) {
    let section = create_preference_group(
        "Blacklisted Packages",
        "Manage the packages added to /etc/pacman.conf IgnorePkg. Pacman will skip updates for these packages until they are removed from the list.",
    );

    let manage_btn = build_padded_button("Manage Blacklisted Packages");
    manage_btn.set_halign(gtk4::Align::Start);
    let parent_clone = parent.clone();
    manage_btn.connect_clicked(move |_| {
        show_manage_blacklist_dialog(parent_clone.upcast_ref::<gtk4::Window>());
    });
    section.append(&manage_btn);

    main_container.append(&section);
}

fn is_flatpak_installed() -> bool {
    return std::process::Command::new("which")
        .arg("flatpak")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
}

fn create_logs_group(settings: &AppSettings, main_container: &gtk4::Box) -> gtk4::SpinButton {
    let section = create_preference_group(
        "Session Logs",
        "How many days of past session logs to keep before they are automatically deleted.",
    );

    let retention_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let retention_label = gtk4::Label::new(Some("Keep logs for (days):"));
    retention_label.set_xalign(0.0);
    retention_label.set_hexpand(true);
    retention_row.append(&retention_label);

    let spin = gtk4::SpinButton::with_range(1.0, 365.0, 1.0);
    spin.set_value(settings.log_retention_days.max(1) as f64);
    spin.set_digits(0);
    retention_row.append(&spin);
    section.append(&retention_row);

    let folder_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    folder_row.set_margin_top(8);
    let folder_label = gtk4::Label::new(Some("Open logs folder"));
    folder_label.set_xalign(0.0);
    folder_label.set_hexpand(true);
    folder_row.append(&folder_label);

    let open_btn = gtk4::Button::from_icon_name("folder-open-symbolic");
    open_btn.set_tooltip_text(Some("Open logs folder"));
    open_btn.add_css_class("flat");
    open_btn.connect_clicked(|_| {
        log_info!("settings: open logs folder clicked");
        open_logs_folder();
    });
    folder_row.append(&open_btn);
    section.append(&folder_row);

    main_container.append(&section);

    return spin;
}

fn create_preference_group(title: &str, description: &str) -> gtk4::Box {
    let group = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    group.add_css_class("preference-group");

    let title_label = gtk4::Label::new(Some(title));
    title_label.set_halign(gtk4::Align::Start);
    title_label.add_css_class("heading");
    title_label.set_markup(&format!("<b>{}</b>", title));
    group.append(&title_label);

    let desc_label = gtk4::Label::new(Some(description));
    desc_label.set_wrap(true);
    desc_label.set_xalign(0.0);
    desc_label.add_css_class("dim-label");
    desc_label.add_css_class("caption");
    desc_label.set_margin_bottom(8);
    group.append(&desc_label);

    return group;
}

fn build_tab_container() -> gtk4::Box {
    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 20);
    container.set_margin_start(24);
    container.set_margin_end(24);
    container.set_margin_top(16);
    container.set_margin_bottom(24);
    return container;
}

fn wrap_tab(content: &gtk4::Box) -> gtk4::ScrolledWindow {
    return gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .child(content)
        .build();
}

fn build_padded_button(label_text: &str) -> gtk4::Button {
    let button = gtk4::Button::new();
    let label = gtk4::Label::new(Some(label_text));
    label.set_margin_start(12);
    label.set_margin_end(12);
    button.set_child(Some(&label));
    return button;
}

fn install_settings_css() {
    use std::sync::OnceLock;
    static CSS_INSTALLED: OnceLock<()> = OnceLock::new();

    CSS_INSTALLED.get_or_init(|| {
        let Some(display) = gtk4::gdk::Display::default() else {
            return;
        };

        let provider = gtk4::CssProvider::new();
        provider.load_from_data(
            "stackswitcher button,
             stackswitcher togglebutton {
                padding-left: 14px;
                padding-right: 14px;
            }",
        );

        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
    });
}

fn pad_switcher_buttons(switcher: &gtk4::StackSwitcher) {
    let mut child = switcher.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        widget.set_hexpand(true);
        widget.set_size_request(110, -1);
        pad_labels_in_widget(&widget);
        child = next;
    }
}

fn pad_labels_in_widget(widget: &gtk4::Widget) {
    if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
        label.set_margin_start(14);
        label.set_margin_end(14);
        return;
    }

    let mut child = widget.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        pad_labels_in_widget(&c);
        child = next;
    }
}
