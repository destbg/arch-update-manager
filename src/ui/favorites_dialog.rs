use crate::helpers::desktop_apps::get_desktop_app_packages;
use crate::helpers::installed_packages::get_all_installed_packages;
use crate::helpers::settings::{load_settings, save_settings};
use crate::helpers::tray_integration::kick_tray;
use crate::log_info;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn show_manage_favorites_dialog(parent: &gtk4::Window) {
    let all_packages = get_all_installed_packages();
    let settings = load_settings();

    let mut favorites: Vec<String> = all_packages
        .iter()
        .filter(|p| settings.is_favorite(p))
        .cloned()
        .collect();
    let mut others: Vec<String> = all_packages
        .iter()
        .filter(|p| !settings.is_favorite(p))
        .cloned()
        .collect();
    favorites.append(&mut others);
    let sorted_packages = favorites;

    let dialog = gtk4::Window::builder()
        .title("Manage Favorite Packages")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(520)
        .build();

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.set_vexpand(true);
    dialog.set_child(Some(&content));

    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some("Search packages"));
    search.set_margin_start(12);
    search.set_margin_end(12);
    search.set_margin_top(12);
    search.set_margin_bottom(8);
    content.append(&search);

    let scrolled = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);

    let checkboxes: Rc<RefCell<Vec<(String, gtk4::CheckButton)>>> =
        Rc::new(RefCell::new(Vec::new()));

    for pkg_name in &sorted_packages {
        let is_fav = settings.is_favorite(pkg_name);

        let row = gtk4::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);

        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(6);
        hbox.set_margin_bottom(6);

        let check = gtk4::CheckButton::new();
        check.set_active(is_fav);

        let label = gtk4::Label::new(Some(pkg_name));
        label.set_halign(gtk4::Align::Start);
        label.set_hexpand(true);

        if is_fav {
            label.add_css_class("heading");
        }

        let check_weak = check.downgrade();
        let click = gtk4::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            if let Some(cb) = check_weak.upgrade() {
                cb.set_active(!cb.is_active());
            }
        });
        label.add_controller(click);

        hbox.append(&check);
        hbox.append(&label);
        row.set_child(Some(&hbox));
        list_box.append(&row);

        checkboxes.borrow_mut().push((pkg_name.clone(), check));
    }

    scrolled.set_child(Some(&list_box));
    content.append(&scrolled);

    let search_clone = search.clone();
    list_box.set_filter_func(move |row| {
        let query = search_clone.text().to_lowercase();
        if query.is_empty() {
            return true;
        }
        row.child()
            .and_downcast::<gtk4::Box>()
            .and_then(|hbox| hbox.last_child().and_downcast::<gtk4::Label>())
            .map(|label| label.text().to_lowercase().contains(&query))
            .unwrap_or(true)
    });

    search.connect_search_changed(move |_| {
        list_box.invalidate_filter();
    });

    let bottom_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    bottom_row.set_halign(gtk4::Align::End);
    bottom_row.set_margin_start(12);
    bottom_row.set_margin_end(12);
    bottom_row.set_margin_top(8);
    bottom_row.set_margin_bottom(12);

    let add_apps_btn = gtk4::Button::with_label("Favorite desktop apps");
    add_apps_btn.set_tooltip_text(Some(
        "Tick every installed package that is a desktop application",
    ));
    bottom_row.append(&add_apps_btn);

    let bulk_btn = if settings.favorites_exclusion_mode {
        let btn = gtk4::Button::with_label("Mark all");
        btn.add_css_class("suggested-action");
        btn.set_tooltip_text(Some("Tick every package in the list"));
        btn
    } else {
        let btn = gtk4::Button::with_label("Clear all");
        btn.add_css_class("destructive-action");
        btn.set_tooltip_text(Some("Uncheck every package in the list"));
        btn
    };
    bottom_row.append(&bulk_btn);

    let close_btn = gtk4::Button::with_label("Close");
    close_btn.add_css_class("suggested-action");
    bottom_row.append(&close_btn);

    content.append(&bottom_row);

    let checkboxes_for_add = checkboxes.clone();
    add_apps_btn.connect_clicked(move |_| {
        log_info!("favorites dialog: Add desktop apps clicked");
        let desktop_apps = get_desktop_app_packages();
        if desktop_apps.is_empty() {
            return;
        }
        for (name, cb) in checkboxes_for_add.borrow().iter() {
            if desktop_apps.contains(name) {
                cb.set_active(true);
            }
        }
    });

    let checkboxes_for_bulk = checkboxes.clone();
    let target_state = settings.favorites_exclusion_mode;
    bulk_btn.connect_clicked(move |_| {
        log_info!("favorites dialog: Bulk toggle clicked");
        for (_, cb) in checkboxes_for_bulk.borrow().iter() {
            cb.set_active(target_state);
        }
    });

    let dialog_for_close = dialog.clone();
    close_btn.connect_clicked(move |_| {
        log_info!("favorites dialog: Close clicked");
        dialog_for_close.close();
    });

    let checkboxes_clone = checkboxes.clone();
    dialog.connect_close_request(move |_| {
        let mut s = load_settings();
        let new_list: Vec<String> = if s.favorites_exclusion_mode {
            checkboxes_clone
                .borrow()
                .iter()
                .filter(|(_, cb)| !cb.is_active())
                .map(|(name, _)| name.clone())
                .collect()
        } else {
            checkboxes_clone
                .borrow()
                .iter()
                .filter(|(_, cb)| cb.is_active())
                .map(|(name, _)| name.clone())
                .collect()
        };
        s.favorite_packages = new_list;
        if let Err(e) = save_settings(&s) {
            eprintln!("Failed to save favorite packages: {}", e);
        } else {
            kick_tray();
        }
        return glib::Propagation::Proceed;
    });

    dialog.present();
}
