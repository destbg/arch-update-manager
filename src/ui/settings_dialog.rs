use gtk4::{ApplicationWindow, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    helpers::pacman_repos::get_repository_groups,
    helpers::settings::{get_available_aur_helpers, load_settings, save_settings},
    models::{app_settings::AppSettings, snapshot_retention_period::SnapshotRetentionPeriod},
};

pub fn show_settings_dialog(parent: &ApplicationWindow, settings: &AppSettings) {
    let dialog = gtk4::Dialog::builder()
        .title("Settings")
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .default_height(500)
        .build();

    let content_area = dialog.content_area();
    content_area.set_spacing(0);
    content_area.set_vexpand(true);

    let scrolled_window = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_container = gtk4::Box::new(gtk4::Orientation::Vertical, 20);
    main_container.set_margin_start(24);
    main_container.set_margin_end(24);
    main_container.set_margin_top(24);
    main_container.set_margin_bottom(24);

    let (aur_enable_check, aur_combo) = create_aur_group(settings, &main_container);
    let (timeshift_check, retention_count_spin, retention_period_combo) =
        create_timeshift_group(settings, &main_container);
    let (separate_repo_check, repo_checkboxes) = create_packages_group(settings, &main_container);

    scrolled_window.set_child(Some(&main_container));
    content_area.append(&scrolled_window);

    let save_all = {
        let aur_enable_check = aur_enable_check.clone();
        let aur_combo = aur_combo.clone();
        let timeshift_check = timeshift_check.clone();
        let retention_count_spin = retention_count_spin.clone();
        let retention_period_combo = retention_period_combo.clone();
        let separate_repo_check = separate_repo_check.clone();
        let repo_checkboxes = repo_checkboxes.clone();

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

            new_settings.separate_repository_groups = separate_repo_check.is_active();

            let mut selected_repos = Vec::new();
            for (repo_id, checkbox) in repo_checkboxes.borrow().iter() {
                if checkbox.is_active() {
                    selected_repos.push(repo_id.clone());
                }
            }
            new_settings.separate_repositories = selected_repos;

            if let Err(e) = save_settings(&new_settings) {
                eprintln!("Failed to save settings: {}", e);
            }
        })
    };

    let aur_combo_weak = aur_combo.clone();
    let save_all_clone = save_all.clone();
    aur_enable_check.connect_toggled(move |check| {
        aur_combo_weak.set_sensitive(check.is_active());
        save_all_clone();
    });

    let save_all_clone = save_all.clone();
    aur_combo.connect_changed(move |_| {
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

    dialog.present();
}

fn create_aur_group(
    settings: &AppSettings,
    main_container: &gtk4::Box,
) -> (gtk4::CheckButton, gtk4::ComboBoxText) {
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
    main_container.append(&aur_section);

    return (aur_enable_check, aur_combo);
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
