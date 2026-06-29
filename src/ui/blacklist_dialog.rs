use gtk4::prelude::*;

use crate::helpers::pacman_ignore::{list_managed_ignores, remove_from_ignore_pkg};
use crate::helpers::tray_integration::trigger_check_service;
use crate::log_info;
use crate::ui::dialogs::build_dialog_window;

pub fn show_manage_blacklist_dialog(parent: &gtk4::Window) {
    let (dialog, content) = build_dialog_window(parent, "Blacklisted Packages", 420, 440);

    let header_label = gtk4::Label::new(Some(
        "Packages listed here are in /etc/pacman.conf IgnorePkg. Pacman will skip updates for these packages until they are removed from the list.",
    ));
    header_label.set_wrap(true);
    header_label.set_xalign(0.0);
    header_label.set_margin_start(16);
    header_label.set_margin_end(16);
    header_label.set_margin_top(12);
    header_label.set_margin_bottom(8);
    header_label.add_css_class("dim-label");
    header_label.add_css_class("caption");
    content.append(&header_label);

    let scrolled = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);
    list_box.set_margin_top(4);
    list_box.set_margin_bottom(12);

    let empty_label = gtk4::Label::new(Some("No packages are currently blacklisted."));
    empty_label.set_wrap(true);
    empty_label.set_xalign(0.5);
    empty_label.set_halign(gtk4::Align::Center);
    empty_label.set_margin_top(24);
    empty_label.set_margin_bottom(24);
    empty_label.add_css_class("dim-label");

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    body.append(&list_box);
    body.append(&empty_label);
    scrolled.set_child(Some(&body));
    content.append(&scrolled);

    let list_box_for_refresh = list_box.clone();
    let empty_label_for_refresh = empty_label.clone();
    let refresh = std::rc::Rc::new(move || {
        rebuild_list(&list_box_for_refresh, &empty_label_for_refresh);
    });
    refresh();

    dialog.present();
}

fn rebuild_list(list_box: &gtk4::ListBox, empty_label: &gtk4::Label) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let entries = list_managed_ignores();

    if entries.is_empty() {
        list_box.set_visible(false);
        empty_label.set_visible(true);
        return;
    }

    list_box.set_visible(true);
    empty_label.set_visible(false);

    for pkg in entries {
        let row = build_row(&pkg, list_box, empty_label);
        list_box.append(&row);
    }
}

fn build_row(pkg: &str, list_box: &gtk4::ListBox, empty_label: &gtk4::Label) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    hbox.set_margin_start(12);
    hbox.set_margin_end(8);
    hbox.set_margin_top(6);
    hbox.set_margin_bottom(6);

    let label = gtk4::Label::new(Some(pkg));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    hbox.append(&label);

    let remove_btn = gtk4::Button::from_icon_name("user-trash-symbolic");
    remove_btn.set_tooltip_text(Some("Remove from blacklist"));
    remove_btn.add_css_class("flat");

    let pkg_clone = pkg.to_string();
    let list_box_clone = list_box.clone();
    let empty_label_clone = empty_label.clone();
    remove_btn.connect_clicked(move |_| {
        log_info!("blacklist dialog: remove {} clicked", pkg_clone);
        if let Err(e) = remove_from_ignore_pkg(&pkg_clone) {
            log_info!("blacklist remove failed for {}: {}", pkg_clone, e);
            eprintln!("Failed to remove {} from IgnorePkg: {}", pkg_clone, e);
            return;
        }
        trigger_check_service();
        rebuild_list(&list_box_clone, &empty_label_clone);
    });
    hbox.append(&remove_btn);

    row.set_child(Some(&hbox));
    return row;
}
