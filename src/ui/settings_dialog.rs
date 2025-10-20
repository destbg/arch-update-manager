use gtk4::{ApplicationWindow, prelude::*};

use crate::{
    helpers::settings::{get_available_aur_helpers, load_settings, save_settings},
    models::app_settings::AppSettings,
};

pub fn show_settings_dialog(parent: &ApplicationWindow, settings: &AppSettings) {
    let dialog = gtk4::Dialog::builder()
        .title("Settings")
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .build();

    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    let ok_button = dialog.add_button("Save", gtk4::ResponseType::Ok);
    ok_button.add_css_class("suggested-action");

    let content_area = dialog.content_area();
    content_area.set_spacing(0);

    let main_container = gtk4::Box::new(gtk4::Orientation::Vertical, 20);
    main_container.set_margin_start(24);
    main_container.set_margin_end(24);
    main_container.set_margin_top(24);
    main_container.set_margin_bottom(24);

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

    let aur_combo_weak = aur_combo.clone();
    aur_enable_check.connect_toggled(move |check| {
        aur_combo_weak.set_sensitive(check.is_active());
    });

    aur_section.append(&aur_combo);
    main_container.append(&aur_section);

    let timeshift_section = create_preference_group(
        "System Snapshots",
        "Automatically create system snapshots before installing updates for easy rollback if needed.",
    );

    let timeshift_check = gtk4::CheckButton::with_label("Create Timeshift snapshot before updates");
    timeshift_check.add_css_class("settings-check");
    timeshift_check.set_active(settings.create_timeshift_snapshot);

    timeshift_section.append(&timeshift_check);
    main_container.append(&timeshift_section);

    content_area.append(&main_container);

    let aur_enable_check_clone = aur_enable_check.clone();
    let aur_combo_clone = aur_combo.clone();
    let timeshift_check_clone = timeshift_check.clone();

    dialog.connect_response(move |dialog, response| {
        if response == gtk4::ResponseType::Ok {
            let mut new_settings = load_settings();

            new_settings.enable_aur_support = aur_enable_check_clone.is_active();

            if let Some(active_id) = aur_combo_clone.active_id() {
                new_settings.preferred_aur_helper = if active_id == "auto" {
                    None
                } else {
                    Some(active_id.to_string())
                };
            }

            new_settings.create_timeshift_snapshot = timeshift_check_clone.is_active();

            if let Err(e) = save_settings(&new_settings) {
                eprintln!("Failed to save settings: {}", e);
            }
        }

        dialog.close();
    });

    dialog.present();
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
