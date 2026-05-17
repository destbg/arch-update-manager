use gtk4::{ApplicationWindow, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    helpers::{
        pacman_repos::get_repository_groups,
        settings::{get_available_aur_helpers, load_settings, save_settings},
    },
    models::{app_settings::AppSettings, snapshot_retention_period::SnapshotRetentionPeriod},
    ui::favorites_dialog,
};

pub fn show_settings_dialog(
    parent: &ApplicationWindow,
    settings: &AppSettings,
    favorites_column: Option<gtk4::ColumnViewColumn>,
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
    let (timeshift_check, retention_count_spin, retention_period_combo) =
        create_timeshift_group(settings, &general_container);
    let snapper_check = create_snapper_group(settings, &general_container);
    let post_update_check = create_post_update_group(settings, &general_container);
    let remember_unselected_check = create_remember_unselected_group(settings, &general_container);
    let detect_switches_check = create_repo_switches_group(settings, &general_container);
    stack.add_titled(&wrap_tab(&general_container), Some("general"), "General");

    let packages_container = build_tab_container();
    let (aur_enable_check, aur_combo, aur_devel_check) =
        create_aur_group(settings, &packages_container);
    let flatpak_enable_check = create_flatpak_group(settings, &packages_container);
    stack.add_titled(&wrap_tab(&packages_container), Some("packages"), "Packages");

    let pacman_container = build_tab_container();
    let (separate_repo_check, repo_checkboxes) = create_packages_group(settings, &pacman_container);
    let (keep_old_spin, keep_uninstalled_spin) = create_cache_group(settings, &pacman_container);
    stack.add_titled(&wrap_tab(&pacman_container), Some("pacman"), "Pacman");

    let favorites_container = build_tab_container();
    let (fav_enable_check, fav_show_col_check, manage_btn) =
        create_favorites_group(settings, &favorites_container, parent);
    stack.add_titled(
        &wrap_tab(&favorites_container),
        Some("favorites"),
        "Favorites",
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
        let detect_switches_check = detect_switches_check.clone();
        let timeshift_check = timeshift_check.clone();
        let retention_count_spin = retention_count_spin.clone();
        let retention_period_combo = retention_period_combo.clone();
        let fav_enable_check = fav_enable_check.clone();
        let fav_show_col_check = fav_show_col_check.clone();
        let separate_repo_check = separate_repo_check.clone();
        let repo_checkboxes = repo_checkboxes.clone();
        let remember_unselected_check = remember_unselected_check.clone();
        let post_update_check = post_update_check.clone();
        let flatpak_enable_check = flatpak_enable_check.clone();
        let keep_old_spin = keep_old_spin.clone();
        let keep_uninstalled_spin = keep_uninstalled_spin.clone();
        let snapper_check = snapper_check.clone();

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

            new_settings.create_timeshift_snapshot = timeshift_check.is_active();
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

            new_settings.detect_repo_switches = detect_switches_check.is_active();

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
            new_settings.create_snapper_snapshot = snapper_check
                .as_ref()
                .map(|c| c.is_active())
                .unwrap_or(false);

            if let Err(e) = save_settings(&new_settings) {
                eprintln!("Failed to save settings: {}", e);
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

    let retention_count_spin_weak = retention_count_spin.clone();
    let retention_period_combo_weak = retention_period_combo.clone();
    let save_all_clone = save_all.clone();
    timeshift_check.connect_toggled(move |check| {
        let is_active = check.is_active();

        if let Some(parent) = retention_count_spin_weak.parent() {
            if let Ok(box_widget) = parent.downcast::<gtk4::Box>() {
                box_widget.set_sensitive(is_active);
            }
        }
        if let Some(parent) = retention_period_combo_weak.parent() {
            if let Ok(box_widget) = parent.downcast::<gtk4::Box>() {
                box_widget.set_sensitive(is_active);
            }
        }

        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    retention_count_spin.connect_value_changed(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    retention_period_combo.connect_changed(move |_| {
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    detect_switches_check.connect_toggled(move |_| {
        save_all_clone();
    });

    if let Some(snapper) = snapper_check.as_ref() {
        let save_all_clone = save_all.clone();
        snapper.connect_toggled(move |_| {
            save_all_clone();
        });
    }

    let fav_show_col_check_weak = fav_show_col_check.downgrade();
    let manage_btn_weak = manage_btn.downgrade();
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

fn create_repo_switches_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> gtk4::CheckButton {
    let section = create_preference_group(
        "Package Resolutions",
        "Detect when locally built packages are available in a sync repository, or when a sync package wants to replace an installed one. Detected resolutions are shown on the post-update checks page.",
    );

    let detect_switches_check = gtk4::CheckButton::with_label("Detect repository switches");
    detect_switches_check.add_css_class("settings-check");
    detect_switches_check.set_active(settings.detect_repo_switches);
    section.append(&detect_switches_check);

    main_container.append(&section);

    return detect_switches_check;
}

fn create_timeshift_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::CheckButton, gtk4::SpinButton, gtk4::ComboBoxText) {
    let timeshift_section = create_preference_group(
        "System Snapshots",
        "Automatically create system snapshots before installing updates for easy rollback if needed.",
    );

    let timeshift_check =
        gtk4::CheckButton::with_label("Create Timeshift snapshot before the update");
    timeshift_check.add_css_class("settings-check");
    timeshift_check.set_active(settings.create_timeshift_snapshot);

    timeshift_section.append(&timeshift_check);

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

    timeshift_section.append(&retention_count_box);

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

    timeshift_section.append(&retention_period_box);

    let deletion_info_label =
        gtk4::Label::new(Some("Old snapshots are only deleted when updating."));
    deletion_info_label.set_wrap(true);
    deletion_info_label.set_xalign(0.0);
    deletion_info_label.set_margin_top(8);
    deletion_info_label.add_css_class("dim-label");
    deletion_info_label.add_css_class("caption");
    timeshift_section.append(&deletion_info_label);

    let is_active = settings.create_timeshift_snapshot;
    retention_count_box.set_sensitive(is_active);
    retention_period_box.set_sensitive(is_active);

    main_container.append(&timeshift_section);

    return (
        timeshift_check,
        retention_count_spin,
        retention_period_combo,
    );
}

fn create_favorites_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
    parent: &ApplicationWindow,
) -> (gtk4::CheckButton, gtk4::CheckButton, gtk4::Button) {
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
        favorites_dialog::show_manage_favorites_dialog(parent_clone.upcast_ref::<gtk4::Window>());
    });
    section.append(&manage_btn);
    main_container.append(&section);

    return (enable_check, show_col_check, manage_btn);
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

fn create_snapper_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> Option<gtk4::CheckButton> {
    if !crate::helpers::snapper::is_snapper_installed() {
        return None;
    }

    let snap_pac_present = crate::helpers::snapper::is_snap_pac_installed();

    let section = create_preference_group(
        "Snapper Snapshots",
        "Create a Snapper snapshot before installing updates. This adds a recovery point you can roll back to if an update breaks the system.",
    );

    let check = gtk4::CheckButton::with_label("Create Snapper snapshot before the update");
    check.add_css_class("settings-check");
    check.set_active(settings.create_snapper_snapshot && !snap_pac_present);
    check.set_sensitive(!snap_pac_present);

    section.append(&check);

    if snap_pac_present {
        let info = gtk4::Label::new(Some(
            "The snap-pac package is installed, so Snapper already creates a snapshot automatically before each pacman transaction. No extra setting is needed.",
        ));
        info.set_wrap(true);
        info.set_xalign(0.0);
        info.set_margin_top(8);
        info.add_css_class("dim-label");
        info.add_css_class("caption");
        section.append(&info);
    }

    main_container.append(&section);

    return Some(check);
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

fn is_flatpak_installed() -> bool {
    return std::process::Command::new("which")
        .arg("flatpak")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
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
