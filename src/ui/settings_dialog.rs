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
        app_settings::AppSettings, aur_managers::AurManagers, check_schedule::CheckSchedule,
        snapshot_group::SnapshotGroup, snapshot_retention_period::SnapshotRetentionPeriod,
    },
    ui::{
        appimage_sources::build_appimage_sources_section,
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

    let dialog = gtk4::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .modal(true)
        .default_width(760)
        .default_height(600)
        .build();

    let content_area = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content_area.set_vexpand(true);
    dialog.set_child(Some(&content_area));

    let updates_container = build_tab_container();
    let (aur_enable_check, aur_combo, aur_devel_check) =
        create_aur_group(settings, &updates_container);
    let flatpak_enable_check = create_flatpak_group(settings, &updates_container);
    let (min_update_age_spin, min_update_age_aur_only_check) =
        create_update_age_group(settings, &updates_container);

    let appimage_container = build_tab_container();
    let appimage_enable_check = create_appimage_section(settings, &appimage_container, parent);

    let repos_container = build_tab_container();
    let (separate_repo_check, repo_checkboxes) = create_packages_group(settings, &repos_container);
    create_blacklist_group(&repos_container, parent);

    let maintenance_container = build_tab_container();
    let snapshot_group = create_snapshot_group(settings, &maintenance_container);
    let (keep_old_spin, keep_uninstalled_spin, auto_clean_cache_check) =
        create_cache_group(settings, &maintenance_container);
    let post_update_check = create_post_update_group(settings, &maintenance_container);

    let tray_container = build_tab_container();
    let (
        system_tray_check,
        always_visible_check,
        only_favorites_check,
        menu_only_favorites_check,
        notify_check,
        check_schedule_combo,
        skip_metered_check,
        skip_battery_check,
    ) = create_system_tray_group(settings, &tray_container);

    let appearance_container = build_tab_container();
    let (show_desc_check, show_updated_check) =
        create_show_descriptions_group(settings, &appearance_container);
    let (fav_enable_check, fav_show_col_check, manage_btn, mode_btn) =
        create_favorites_group(settings, &appearance_container, parent);
    let remember_unselected_check =
        create_remember_unselected_group(settings, &appearance_container);

    let topbar_container = build_tab_container();
    let news_check = create_news_group(settings, &topbar_container);
    let mirror_refresh_check = create_mirror_group(settings, &topbar_container);

    let system_container = build_tab_container();
    let log_retention_spin = create_logs_group(settings, &system_container);

    let categories: [(&str, &str); 8] = [
        ("Updates", "software-update-available-symbolic"),
        ("AppImages", "application-x-executable-symbolic"),
        ("Repositories", "drive-harddisk-symbolic"),
        ("Snapshots & Cache", "document-save-symbolic"),
        (
            "Tray & Notifications",
            "preferences-system-notifications-symbolic",
        ),
        ("Appearance", "preferences-desktop-appearance-symbolic"),
        ("Top Bar", "open-menu-symbolic"),
        ("System", "emblem-system-symbolic"),
    ];

    let containers = [
        updates_container,
        appimage_container,
        repos_container,
        maintenance_container,
        tray_container,
        appearance_container,
        topbar_container,
        system_container,
    ];

    let content_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    for container in &containers {
        content_vbox.append(container);
    }

    let content_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&content_vbox)
        .build();

    let mut card_records: Vec<(usize, gtk4::Widget, String)> = Vec::new();
    for (index, container) in containers.iter().enumerate() {
        let mut child = container.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            let text = collect_label_text(&widget).to_lowercase();
            card_records.push((index, widget.clone(), text));
            child = next;
        }
    }

    let card_records = Rc::new(card_records);
    let containers = Rc::new(containers);
    let selected_category = Rc::new(std::cell::Cell::new(0usize));

    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search settings"));
    search_entry.set_hexpand(true);
    search_entry.set_margin_start(12);
    search_entry.set_margin_end(12);
    search_entry.set_margin_top(10);
    search_entry.set_margin_bottom(10);

    let apply_filter: Rc<dyn Fn(&str)> = {
        let card_records = card_records.clone();
        let containers = containers.clone();
        let selected_category = selected_category.clone();
        Rc::new(move |query: &str| {
            let query = query.trim().to_lowercase();
            if query.is_empty() {
                let selected = selected_category.get();
                for (index, container) in containers.iter().enumerate() {
                    for (card_index, card, _) in card_records.iter() {
                        if *card_index == index {
                            card.set_visible(true);
                        }
                    }
                    container.set_visible(index == selected);
                }
            } else {
                for (index, container) in containers.iter().enumerate() {
                    let mut any_visible = false;
                    for (card_index, card, text) in card_records.iter() {
                        if *card_index == index {
                            let matches = text.contains(&query);
                            card.set_visible(matches);
                            any_visible |= matches;
                        }
                    }
                    container.set_visible(any_visible);
                }
            }
        })
    };

    let sidebar_list = gtk4::ListBox::new();
    sidebar_list.add_css_class("navigation-sidebar");
    sidebar_list.add_css_class("settings-sidebar");
    sidebar_list.set_vexpand(true);
    sidebar_list.set_size_request(210, -1);

    for (title, icon) in categories {
        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        row_box.set_margin_top(10);
        row_box.set_margin_bottom(10);
        row_box.set_margin_start(6);
        row_box.set_margin_end(6);

        let image = gtk4::Image::from_icon_name(icon);
        row_box.append(&image);

        let label = gtk4::Label::new(Some(title));
        label.set_xalign(0.0);
        row_box.append(&label);

        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(&row_box));
        sidebar_list.append(&row);
    }

    let apply_for_nav = apply_filter.clone();
    let selected_for_nav = selected_category.clone();
    let search_for_nav = search_entry.clone();
    sidebar_list.connect_row_selected(move |_, row| {
        let Some(row) = row else {
            return;
        };
        selected_for_nav.set(row.index() as usize);
        apply_for_nav(&search_for_nav.text());
    });
    sidebar_list.select_row(sidebar_list.row_at_index(0).as_ref());

    let apply_for_search = apply_filter.clone();
    search_entry.connect_search_changed(move |entry| {
        apply_for_search(&entry.text());
    });

    let content_side = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content_side.set_hexpand(true);
    content_side.append(&search_entry);
    content_side.append(&content_scroll);

    let nav = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    nav.set_vexpand(true);
    nav.append(&sidebar_list);
    nav.append(&content_side);

    content_area.append(&nav);

    let aur_combo_for_enable = aur_combo.clone();
    let aur_devel_for_enable = aur_devel_check.clone();
    aur_enable_check.connect_toggled(move |check| {
        let is_active = check.is_active();
        aur_combo_for_enable.set_sensitive(is_active);
        update_devel_sensitivity(check, &aur_combo_for_enable, &aur_devel_for_enable);
        update_settings(move |s| s.enable_aur_support = is_active);
    });

    let aur_enable_for_combo = aur_enable_check.clone();
    let aur_devel_for_combo = aur_devel_check.clone();
    aur_combo.connect_selected_notify(move |combo| {
        let value =
            dropdown_active_id(combo).and_then(|id| if id == "auto" { None } else { Some(id) });
        update_devel_sensitivity(&aur_enable_for_combo, combo, &aur_devel_for_combo);
        update_settings(move |s| s.preferred_aur_helper = value);
    });

    aur_devel_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.enable_devel_aur = value);
    });

    wire_snapshot_group_signals(&snapshot_group);

    let fav_show_col_for_enable = fav_show_col_check.downgrade();
    let manage_btn_weak = manage_btn.downgrade();
    let mode_btn_weak = mode_btn.downgrade();
    let favorites_column_for_enable = favorites_column.clone();
    fav_enable_check.connect_toggled(move |check| {
        let is_enabled = check.is_active();
        if let Some(col) = &favorites_column_for_enable {
            let show_col = fav_show_col_for_enable
                .upgrade()
                .map(|c| c.is_active())
                .unwrap_or(false);
            col.set_visible(is_enabled && show_col);
        }
        if let Some(c) = fav_show_col_for_enable.upgrade() {
            c.set_sensitive(is_enabled);
        }
        if let Some(btn) = manage_btn_weak.upgrade() {
            btn.set_sensitive(is_enabled);
        }
        if let Some(btn) = mode_btn_weak.upgrade() {
            btn.set_sensitive(is_enabled);
        }
        update_settings(move |s| s.enable_favorites = is_enabled);
    });

    let fav_enable_for_col = fav_enable_check.downgrade();
    let favorites_column_for_col = favorites_column.clone();
    fav_show_col_check.connect_toggled(move |check| {
        let active = check.is_active();
        if let Some(col) = &favorites_column_for_col {
            let is_enabled = fav_enable_for_col
                .upgrade()
                .map(|c| c.is_active())
                .unwrap_or(false);
            col.set_visible(is_enabled && active);
        }
        update_settings(move |s| s.show_favorites_column = active);
    });

    let repo_checkboxes_for_enable = repo_checkboxes.clone();
    separate_repo_check.connect_toggled(move |check| {
        let is_active = check.is_active();
        for (_, checkbox) in repo_checkboxes_for_enable.borrow().iter() {
            checkbox.set_sensitive(is_active);
        }
        update_settings(move |s| s.separate_repository_groups = is_active);
    });

    for (_, checkbox) in repo_checkboxes.borrow().iter() {
        let repo_checkboxes_for_save = repo_checkboxes.clone();
        checkbox.connect_toggled(move |_| {
            let selected: Vec<String> = repo_checkboxes_for_save
                .borrow()
                .iter()
                .filter(|(_, c)| c.is_active())
                .map(|(id, _)| id.clone())
                .collect();
            update_settings(move |s| s.separate_repositories = selected);
        });
    }

    remember_unselected_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.remember_unselected_packages = value);
    });

    news_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.check_arch_news = value);
    });

    mirror_refresh_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.enable_mirror_refresh = value);
    });

    post_update_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.run_post_update_checks = value);
    });

    flatpak_enable_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.enable_flatpak_support = value);
    });

    appimage_enable_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.enable_appimage_support = value);
    });

    keep_old_spin.connect_value_changed(move |spin| {
        let value = spin.value() as u32;
        update_settings(move |s| s.keep_old_packages = value);
    });

    keep_uninstalled_spin.connect_value_changed(move |spin| {
        let value = spin.value() as u32;
        update_settings(move |s| s.keep_uninstalled_packages = value);
    });

    auto_clean_cache_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.auto_clean_cache = value);
    });

    let notify_for_tray = notify_check.clone();
    let always_visible_for_tray = always_visible_check.clone();
    let only_favorites_for_tray = only_favorites_check.clone();
    let menu_only_for_tray = menu_only_favorites_check.clone();
    let schedule_for_tray = check_schedule_combo.clone();
    let skip_metered_for_tray = skip_metered_check.clone();
    let skip_battery_for_tray = skip_battery_check.clone();
    system_tray_check.connect_toggled(move |check| {
        let active = check.is_active();
        notify_for_tray.set_sensitive(active);
        always_visible_for_tray.set_sensitive(active);
        only_favorites_for_tray.set_sensitive(active);
        menu_only_for_tray.set_sensitive(active);
        schedule_for_tray.set_sensitive(active);
        skip_metered_for_tray.set_sensitive(active);
        skip_battery_for_tray.set_sensitive(active);

        let always = always_visible_for_tray.is_active();
        let only_fav = only_favorites_for_tray.is_active();
        let menu_only = menu_only_for_tray.is_active();
        let notify = notify_for_tray.is_active();
        let metered = skip_metered_for_tray.is_active();
        let battery = skip_battery_for_tray.is_active();
        update_settings(move |s| {
            s.enable_system_tray = active;
            s.tray_always_visible = active && always;
            s.tray_only_favorites = active && only_fav;
            s.tray_menu_only_favorites = active && menu_only;
            s.show_update_notifications = active && notify;
            s.skip_check_on_metered = active && metered;
            s.skip_check_on_battery = active && battery;
        });

        if active {
            let schedule = dropdown_active_id(&schedule_for_tray)
                .map(|id| CheckSchedule::from_id(&id))
                .unwrap_or_default();
            apply_check_schedule(schedule);
        }
        apply_tray_state(active);
    });

    let system_tray_for_notify = system_tray_check.clone();
    notify_check.connect_toggled(move |check| {
        let value = system_tray_for_notify.is_active() && check.is_active();
        update_settings(move |s| s.show_update_notifications = value);
    });

    let only_favorites_for_excl = only_favorites_check.clone();
    let system_tray_for_always = system_tray_check.clone();
    always_visible_check.connect_toggled(move |btn| {
        if btn.is_active() && only_favorites_for_excl.is_active() {
            only_favorites_for_excl.set_active(false);
        }
        let value = system_tray_for_always.is_active() && btn.is_active();
        update_settings(move |s| s.tray_always_visible = value);
        kick_tray();
    });

    let always_visible_for_excl = always_visible_check.clone();
    let system_tray_for_only = system_tray_check.clone();
    only_favorites_check.connect_toggled(move |btn| {
        if btn.is_active() && always_visible_for_excl.is_active() {
            always_visible_for_excl.set_active(false);
        }
        let value = system_tray_for_only.is_active() && btn.is_active();
        update_settings(move |s| s.tray_only_favorites = value);
        kick_tray();
    });

    let system_tray_for_menu = system_tray_check.clone();
    menu_only_favorites_check.connect_toggled(move |btn| {
        let value = system_tray_for_menu.is_active() && btn.is_active();
        update_settings(move |s| s.tray_menu_only_favorites = value);
        kick_tray();
    });

    let system_tray_for_metered = system_tray_check.clone();
    skip_metered_check.connect_toggled(move |btn| {
        let value = system_tray_for_metered.is_active() && btn.is_active();
        update_settings(move |s| s.skip_check_on_metered = value);
    });

    let system_tray_for_battery = system_tray_check.clone();
    skip_battery_check.connect_toggled(move |btn| {
        let value = system_tray_for_battery.is_active() && btn.is_active();
        update_settings(move |s| s.skip_check_on_battery = value);
    });

    check_schedule_combo.connect_selected_notify(move |combo| {
        let schedule = dropdown_active_id(combo)
            .map(|id| CheckSchedule::from_id(&id))
            .unwrap_or_default();
        let schedule_for_save = schedule.clone();
        update_settings(move |s| s.check_schedule = schedule_for_save);
        apply_check_schedule(schedule);
    });

    log_retention_spin.connect_value_changed(move |spin| {
        let value = spin.value() as u32;
        update_settings(move |s| s.log_retention_days = value);
    });

    let package_store_for_desc = package_store.clone();
    show_desc_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.show_package_descriptions = value);
        if let Some(store) = &package_store_for_desc {
            let n = store.n_items();
            if n > 0 {
                store.items_changed(0, n, n);
            }
        }
    });

    let package_store_for_updated = package_store.clone();
    show_updated_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.show_updated_date = value);
        if let Some(store) = &package_store_for_updated {
            let n = store.n_items();
            if n > 0 {
                store.items_changed(0, n, n);
            }
        }
    });

    let aur_only_for_age = min_update_age_aur_only_check.clone();
    min_update_age_spin.connect_value_changed(move |spin| {
        let value = spin.value() as u32;
        aur_only_for_age.set_sensitive(spin.value() > 0.0);
        update_settings(move |s| s.min_update_age_days = value);
    });

    min_update_age_aur_only_check.connect_toggled(move |check| {
        let value = check.is_active();
        update_settings(move |s| s.min_update_age_aur_only = value);
    });

    dialog.present();
}

fn update_settings(apply: impl FnOnce(&mut AppSettings)) {
    let mut settings = load_settings();
    apply(&mut settings);
    if let Err(e) = save_settings(&settings) {
        log_info!("failed to save settings: {}", e);
        eprintln!("Failed to save settings: {}", e);
    }
}

fn collect_label_text(widget: &gtk4::Widget) -> String {
    let mut text = String::new();
    collect_labels_into(widget, &mut text);
    return text;
}

fn collect_labels_into(widget: &gtk4::Widget, out: &mut String) {
    if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
        out.push_str(&label.text());
        out.push(' ');
    }

    let mut child = widget.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        collect_labels_into(&c, out);
        child = next;
    }
}

fn build_id_dropdown(entries: &[(String, String)]) -> gtk4::DropDown {
    let labels: Vec<&str> = entries.iter().map(|(_, label)| label.as_str()).collect();
    let dropdown = gtk4::DropDown::from_strings(&labels);
    let ids: Vec<String> = entries.iter().map(|(id, _)| id.clone()).collect();
    unsafe {
        dropdown.set_data("ids", ids);
    }
    return dropdown;
}

fn dropdown_active_id(dropdown: &gtk4::DropDown) -> Option<String> {
    unsafe {
        let ids = dropdown.data::<Vec<String>>("ids")?;
        return ids.as_ref().get(dropdown.selected() as usize).cloned();
    }
}

fn dropdown_set_active_id(dropdown: &gtk4::DropDown, id: &str) {
    unsafe {
        if let Some(ids) = dropdown.data::<Vec<String>>("ids") {
            if let Some(position) = ids.as_ref().iter().position(|candidate| candidate == id) {
                dropdown.set_selected(position as u32);
            }
        }
    }
}

fn create_aur_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::CheckButton, gtk4::DropDown, gtk4::CheckButton) {
    let aur_section = create_preference_group(
        "AUR Package Manager",
        "Enable support for installing packages from the Arch User Repository (AUR).",
    );

    let aur_enable_check = gtk4::CheckButton::with_label("Enable AUR support");
    aur_enable_check.add_css_class("settings-check");
    aur_enable_check.set_active(settings.enable_aur_support);
    aur_section.append(&aur_enable_check);

    let available_helpers = get_available_aur_helpers();
    let mut aur_entries = vec![("auto".to_string(), "Auto-detect".to_string())];
    for helper in &available_helpers {
        aur_entries.push((helper.clone(), helper.clone()));
    }
    let aur_combo = build_id_dropdown(&aur_entries);
    aur_combo.add_css_class("settings-combo");
    aur_combo.set_margin_top(8);
    dropdown_set_active_id(
        &aur_combo,
        settings.preferred_aur_helper.as_deref().unwrap_or("auto"),
    );
    aur_combo.set_sensitive(settings.enable_aur_support);
    aur_section.append(&aur_combo);

    let devel_check = gtk4::CheckButton::with_label("Check development packages (devel mode)");
    devel_check.add_css_class("settings-check");
    devel_check.set_active(settings.enable_devel_aur);
    devel_check.set_margin_top(12);
    update_devel_sensitivity(&aur_enable_check, &aur_combo, &devel_check);
    aur_section.append(&devel_check);

    main_container.append(&aur_section);

    return (aur_enable_check, aur_combo, devel_check);
}

fn aur_helper_for_selection(combo: &gtk4::DropDown) -> Option<String> {
    let id = dropdown_active_id(combo).unwrap_or_else(|| "auto".to_string());
    if id == "auto" {
        return get_available_aur_helpers().into_iter().next();
    }
    return Some(id);
}

fn selection_supports_devel(combo: &gtk4::DropDown) -> bool {
    return aur_helper_for_selection(combo)
        .and_then(|name| AurManagers::from_command(&name))
        .map(|manager| manager.supports_devel())
        .unwrap_or(false);
}

fn update_devel_sensitivity(
    enable_check: &gtk4::CheckButton,
    combo: &gtk4::DropDown,
    devel_check: &gtk4::CheckButton,
) {
    let supported = selection_supports_devel(combo);
    devel_check.set_sensitive(enable_check.is_active() && supported);
    if supported {
        devel_check.set_tooltip_text(Some(
            "Also check git, svn, and bzr packages for new commits, not just version bumps.",
        ));
    } else {
        devel_check.set_tooltip_text(Some("The selected AUR helper does not support devel mode."));
    }
}

fn create_show_descriptions_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::CheckButton, gtk4::CheckButton) {
    let section = create_preference_group(
        "Package List Display",
        "Show a short description under each package name in the update list.",
    );

    let check = gtk4::CheckButton::with_label("Show package descriptions");
    check.add_css_class("settings-check");
    check.set_active(settings.show_package_descriptions);
    section.append(&check);

    let updated_check = gtk4::CheckButton::with_label("Show last updated date");
    updated_check.add_css_class("settings-check");
    updated_check.set_active(settings.show_updated_date);
    updated_check.set_margin_top(8);
    section.append(&updated_check);

    main_container.append(&section);

    return (check, updated_check);
}

fn create_update_age_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::SpinButton, gtk4::CheckButton) {
    let section = create_preference_group(
        "Update Delay",
        "Wait a number of days before a new update shows up, so it can settle before you install it.",
    );

    let age_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    age_box.set_hexpand(true);

    let age_label = gtk4::Label::new(Some("Hide updates newer than (days)"));
    age_label.set_halign(gtk4::Align::Start);
    age_label.set_hexpand(true);
    age_label.set_tooltip_text(Some(
        "Set to 0 to show all updates. Packages with no known date are always shown.",
    ));
    age_box.append(&age_label);

    let age_spin = gtk4::SpinButton::with_range(0.0, 365.0, 1.0);
    age_spin.set_value(settings.min_update_age_days as f64);
    age_spin.add_css_class("settings-spin");
    age_spin.set_halign(gtk4::Align::End);
    age_box.append(&age_spin);

    section.append(&age_box);

    let aur_only_check = gtk4::CheckButton::with_label("Apply only to AUR packages");
    aur_only_check.add_css_class("settings-check");
    aur_only_check.set_active(settings.min_update_age_aur_only);
    aur_only_check.set_sensitive(settings.min_update_age_days > 0);
    aur_only_check.set_margin_top(8);
    section.append(&aur_only_check);

    main_container.append(&section);

    return (age_spin, aur_only_check);
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
    gtk4::DropDown,
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

    let schedule_entries: Vec<(String, String)> = CheckSchedule::all()
        .iter()
        .map(|schedule| (schedule.id().to_string(), schedule.label().to_string()))
        .collect();
    let check_schedule_combo = build_id_dropdown(&schedule_entries);
    check_schedule_combo.add_css_class("settings-combo");
    dropdown_set_active_id(&check_schedule_combo, settings.check_schedule.id());
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

    let mut provider_entries: Vec<(String, String)> = Vec::new();
    if has_timeshift {
        provider_entries.push(("timeshift".to_string(), "Timeshift".to_string()));
    }
    if has_snapper {
        provider_entries.push(("snapper".to_string(), "Snapper".to_string()));
    }
    let provider_combo = build_id_dropdown(&provider_entries);
    provider_combo.add_css_class("settings-combo");

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
        dropdown_set_active_id(&provider_combo, active_provider);
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

    let retention_period_combo = build_id_dropdown(&[
        ("forever".to_string(), "Forever".to_string()),
        ("day".to_string(), "1 Day".to_string()),
        ("week".to_string(), "1 Week".to_string()),
        ("month".to_string(), "1 Month".to_string()),
        ("year".to_string(), "1 Year".to_string()),
    ]);
    retention_period_combo.add_css_class("settings-combo");

    let active_id = match settings.snapshot_retention_period {
        SnapshotRetentionPeriod::Forever => "forever",
        SnapshotRetentionPeriod::Day => "day",
        SnapshotRetentionPeriod::Week => "week",
        SnapshotRetentionPeriod::Month => "month",
        SnapshotRetentionPeriod::Year => "year",
    };
    dropdown_set_active_id(&retention_period_combo, active_id);
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

    let selected_timeshift = dropdown_active_id(&provider_combo).as_deref() == Some("timeshift");
    let selected_snapper = dropdown_active_id(&provider_combo).as_deref() == Some("snapper");

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

fn save_snapshot_settings(group: &SnapshotGroup) {
    let enabled = group.enable_check.is_active();
    let is_timeshift = dropdown_active_id(&group.provider_combo).as_deref() == Some("timeshift");
    let is_snapper = dropdown_active_id(&group.provider_combo).as_deref() == Some("snapper");
    let count = group.retention_count_spin.value() as u32;
    let period = match dropdown_active_id(&group.retention_period_combo).as_deref() {
        Some("day") => SnapshotRetentionPeriod::Day,
        Some("week") => SnapshotRetentionPeriod::Week,
        Some("month") => SnapshotRetentionPeriod::Month,
        Some("year") => SnapshotRetentionPeriod::Year,
        _ => SnapshotRetentionPeriod::Forever,
    };

    update_settings(move |s| {
        s.create_timeshift_snapshot = enabled && is_timeshift;
        s.create_snapper_snapshot = enabled && is_snapper;
        s.snapshot_retention_count = count;
        s.snapshot_retention_period = period;
    });
}

fn wire_snapshot_group_signals(group: &SnapshotGroup) {
    let save: Rc<dyn Fn()> = {
        let group = group.clone();
        Rc::new(move || save_snapshot_settings(&group))
    };

    let provider_combo_w = group.provider_combo.clone();
    let retention_count_box_w = group.retention_count_box.clone();
    let retention_period_box_w = group.retention_period_box.clone();
    let deletion_info_label_w = group.deletion_info_label.clone();
    let snap_pac_info_w = group.snap_pac_info.clone();
    let snap_pac_installed = group.snap_pac_installed;
    let save_clone = save.clone();
    group.enable_check.connect_toggled(move |check| {
        let enabled = check.is_active();
        provider_combo_w.set_sensitive(enabled);
        let is_timeshift = dropdown_active_id(&provider_combo_w).as_deref() == Some("timeshift");
        let is_snapper = dropdown_active_id(&provider_combo_w).as_deref() == Some("snapper");
        retention_count_box_w.set_sensitive(enabled && is_timeshift);
        retention_period_box_w.set_sensitive(enabled && is_timeshift);
        retention_count_box_w.set_visible(is_timeshift);
        retention_period_box_w.set_visible(is_timeshift);
        deletion_info_label_w.set_visible(is_timeshift);
        snap_pac_info_w.set_visible(enabled && is_snapper && snap_pac_installed);
        save_clone();
    });

    let enable_check_w = group.enable_check.clone();
    let retention_count_box_w = group.retention_count_box.clone();
    let retention_period_box_w = group.retention_period_box.clone();
    let deletion_info_label_w = group.deletion_info_label.clone();
    let snap_pac_info_w = group.snap_pac_info.clone();
    let save_clone = save.clone();
    group.provider_combo.connect_selected_notify(move |combo| {
        let enabled = enable_check_w.is_active();
        let is_timeshift = dropdown_active_id(combo).as_deref() == Some("timeshift");
        let is_snapper = dropdown_active_id(combo).as_deref() == Some("snapper");
        retention_count_box_w.set_sensitive(enabled && is_timeshift);
        retention_period_box_w.set_sensitive(enabled && is_timeshift);
        retention_count_box_w.set_visible(is_timeshift);
        retention_period_box_w.set_visible(is_timeshift);
        deletion_info_label_w.set_visible(is_timeshift);
        snap_pac_info_w.set_visible(enabled && is_snapper && snap_pac_installed);
        save_clone();
    });

    let save_clone = save.clone();
    group
        .retention_count_spin
        .connect_value_changed(move |_| save_clone());

    let save_clone = save.clone();
    group
        .retention_period_combo
        .connect_selected_notify(move |_| save_clone());
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
        let btn_for_response = btn.clone();
        show_confirm_dialog(&parent_for_mode, title, message, accept_label, move |accepted| {
            if !accepted {
                return;
            }
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

fn create_mirror_group(settings: &AppSettings, main_container: &gtk4::Box) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Mirror List",
        "Show a banner that offers to refresh the pacman mirror list when it gets old.",
    );

    let check = gtk4::CheckButton::with_label("Offer to refresh the mirror list");
    check.add_css_class("settings-check");
    check.set_active(settings.enable_mirror_refresh);

    section.append(&check);
    main_container.append(&section);

    return check;
}

fn create_news_group(settings: &AppSettings, main_container: &gtk4::Box) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Arch Linux News",
        "Check the Arch Linux news feed when the app starts and show any new posts.",
    );

    let check = gtk4::CheckButton::with_label("Check for news on startup");
    check.add_css_class("settings-check");
    check.set_active(settings.check_arch_news);

    section.append(&check);
    main_container.append(&section);

    return check;
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

fn create_appimage_section(
    settings: &AppSettings,
    main_container: &gtk4::Box,
    parent: &ApplicationWindow,
) -> gtk4::CheckButton {
    let section = create_preference_group(
        "AppImage Packages",
        "Show updates for your AppImages next to system packages.",
    );

    let check = gtk4::CheckButton::with_label("Enable AppImage support");
    check.add_css_class("settings-check");
    check.set_active(settings.enable_appimage_support);

    section.append(&check);
    main_container.append(&section);

    let sources_section = create_preference_group(
        "Update Sources",
        "Pick where each AppImage looks for new versions. AppImages with no source are not checked.",
    );
    sources_section.append(&build_appimage_sources_section(parent));
    main_container.append(&sources_section);

    return check;
}

fn create_cache_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::SpinButton, gtk4::SpinButton, gtk4::CheckButton) {
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

    let auto_clean_check =
        gtk4::CheckButton::with_label("Clean the cache automatically after each update");
    auto_clean_check.add_css_class("settings-check");
    auto_clean_check.set_active(settings.auto_clean_cache);
    auto_clean_check.set_margin_top(8);
    section.append(&auto_clean_check);

    main_container.append(&section);

    return (old_spin, uninst_spin, auto_clean_check);
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

fn install_settings_css() {
    use std::sync::OnceLock;
    static CSS_INSTALLED: OnceLock<()> = OnceLock::new();

    CSS_INSTALLED.get_or_init(|| {
        let Some(display) = gtk4::gdk::Display::default() else {
            return;
        };

        let provider = gtk4::CssProvider::new();
        provider.load_from_data(
            ".settings-card {
                background-color: alpha(currentColor, 0.05);
                border: 1px solid alpha(currentColor, 0.08);
                border-radius: 12px;
                padding: 16px;
            }
            .settings-sidebar {
                background-color: alpha(currentColor, 0.05);
            }",
        );

        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}

fn create_preference_group(title: &str, description: &str) -> gtk4::Box {
    let group = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    group.add_css_class("settings-card");

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

fn build_padded_button(label_text: &str) -> gtk4::Button {
    let button = gtk4::Button::new();
    let label = gtk4::Label::new(Some(label_text));
    label.set_margin_start(12);
    label.set_margin_end(12);
    button.set_child(Some(&label));
    return button;
}
