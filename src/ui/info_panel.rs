use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, Separator, ToggleButton};
use std::cell::RefCell;
use std::rc::Rc;

use crate::helpers::elevated::open_url_as_user;
use crate::log_info;
use crate::models::info_panel::InfoPanel;
use crate::ui::aur_scan_dialog::show_aur_scan_dialog;
use crate::ui::pkgbuild_review_dialog::show_pkgbuild_review_dialog;

pub fn create_info_panel() -> InfoPanel {
    let info_box = GtkBox::new(Orientation::Vertical, 6);
    info_box.set_margin_start(12);
    info_box.set_margin_end(12);
    info_box.set_margin_top(6);
    info_box.set_margin_bottom(6);
    info_box.set_visible(false);

    let header = GtkBox::new(Orientation::Horizontal, 6);

    let title_box = GtkBox::new(Orientation::Vertical, 2);
    title_box.set_hexpand(true);
    title_box.set_valign(Align::Center);

    let title_label = Label::new(Some("Information"));
    title_label.set_xalign(0.0);
    title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title_label.add_css_class("heading");
    title_box.append(&title_label);

    let created_label = Label::new(None);
    created_label.set_xalign(0.0);
    created_label.set_wrap(true);
    created_label.add_css_class("dim-label");
    created_label.add_css_class("caption");
    created_label.set_visible(false);
    created_label.connect_activate_link(|_, uri| {
        open_url_as_user(uri);
        return glib::Propagation::Stop;
    });
    title_box.append(&created_label);

    let maintainer_label = Label::new(None);
    maintainer_label.set_xalign(0.0);
    maintainer_label.set_wrap(true);
    maintainer_label.add_css_class("caption");
    maintainer_label.set_visible(false);
    title_box.append(&maintainer_label);

    let permissions_label = Label::new(None);
    permissions_label.set_xalign(0.0);
    permissions_label.set_wrap(true);
    permissions_label.add_css_class("caption");
    permissions_label.set_visible(false);
    title_box.append(&permissions_label);

    let deps_label = Label::new(None);
    deps_label.set_xalign(0.0);
    deps_label.set_wrap(true);
    deps_label.add_css_class("dim-label");
    deps_label.add_css_class("caption");
    deps_label.set_visible(false);
    title_box.append(&deps_label);

    header.append(&title_box);

    let ignore_button = ToggleButton::new();
    ignore_button.set_icon_name("action-unavailable-symbolic");
    ignore_button.set_tooltip_text(Some("Add to pacman IgnorePkg blacklist"));
    ignore_button.add_css_class("flat");
    ignore_button.set_halign(Align::End);
    ignore_button.set_visible(false);
    header.append(&ignore_button);

    let pkgbuild_button = Button::from_icon_name("applications-engineering-symbolic");
    pkgbuild_button.set_tooltip_text(Some("Review PKGBUILD changes"));
    pkgbuild_button.add_css_class("flat");
    pkgbuild_button.set_halign(Align::End);
    pkgbuild_button.set_visible(false);
    header.append(&pkgbuild_button);

    let aur_scan_button = Button::from_icon_name("security-high-symbolic");
    aur_scan_button.set_tooltip_text(Some("View aur-scan results"));
    aur_scan_button.add_css_class("flat");
    aur_scan_button.set_halign(Align::End);
    aur_scan_button.set_visible(false);
    header.append(&aur_scan_button);

    let release_notes_button = Button::from_icon_name("emblem-documents-symbolic");
    release_notes_button.set_tooltip_text(Some("Open release notes"));
    release_notes_button.add_css_class("flat");
    release_notes_button.set_halign(Align::End);
    release_notes_button.set_visible(false);
    header.append(&release_notes_button);

    let url_button = Button::from_icon_name("web-browser-symbolic");
    url_button.set_tooltip_text(Some("Open homepage"));
    url_button.add_css_class("flat");
    url_button.set_halign(Align::End);
    url_button.set_visible(false);
    header.append(&url_button);

    info_box.append(&header);

    let content_box = GtkBox::new(Orientation::Vertical, 6);
    content_box.set_vexpand(true);

    let separator = Separator::new(Orientation::Horizontal);
    content_box.append(&separator);

    let info_text = Label::new(None);
    info_text.set_xalign(0.0);
    info_text.set_yalign(0.0);
    info_text.set_wrap(true);
    info_text.set_wrap_mode(gtk4::pango::WrapMode::Word);
    info_text.set_hexpand(true);
    info_text.set_vexpand(false);

    let scrolled_window = gtk4::ScrolledWindow::new();
    scrolled_window.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scrolled_window.set_child(Some(&info_text));
    scrolled_window.set_hexpand(true);
    scrolled_window.set_vexpand(true);

    content_box.append(&scrolled_window);

    info_box.append(&content_box);

    let current_url: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let current_release_notes_url: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let current_package: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let current_url_clone = current_url.clone();
    url_button.connect_clicked(move |_| {
        if let Some(url) = current_url_clone.borrow().clone() {
            log_info!("info panel: open homepage {}", url);
            open_url_as_user(&url);
        }
    });

    let current_release_notes_url_clone = current_release_notes_url.clone();
    release_notes_button.connect_clicked(move |_| {
        if let Some(url) = current_release_notes_url_clone.borrow().clone() {
            log_info!("info panel: open release notes {}", url);
            open_url_as_user(&url);
        }
    });

    let current_package_for_pkgbuild = current_package.clone();
    pkgbuild_button.connect_clicked(move |btn| {
        let Some(package) = current_package_for_pkgbuild.borrow().clone() else {
            return;
        };
        let Some(window) = btn.root().and_downcast::<gtk4::Window>() else {
            return;
        };
        log_info!("info panel: review PKGBUILD {}", package);
        show_pkgbuild_review_dialog(&window, &package);
    });

    let current_package_for_scan = current_package.clone();
    aur_scan_button.connect_clicked(move |btn| {
        let Some(package) = current_package_for_scan.borrow().clone() else {
            return;
        };
        let Some(window) = btn.root().and_downcast::<gtk4::Window>() else {
            return;
        };
        log_info!("info panel: view aur-scan results {}", package);
        show_aur_scan_dialog(&window, &package);
    });

    let ignore_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));

    return InfoPanel {
        container: info_box,
        title_label,
        created_label,
        maintainer_label,
        permissions_label,
        deps_label,
        info_text,
        url_button,
        release_notes_button,
        pkgbuild_button,
        aur_scan_button,
        ignore_button,
        ignore_handler_id,
        current_url,
        current_release_notes_url,
        current_package,
    };
}

pub fn update_ignore_button_tooltip(btn: &ToggleButton) {
    if btn.is_active() {
        btn.set_tooltip_text(Some("Remove from pacman IgnorePkg blacklist"));
    } else {
        btn.set_tooltip_text(Some("Add to pacman IgnorePkg blacklist"));
    }
}
