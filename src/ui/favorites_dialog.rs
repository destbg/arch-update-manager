use crate::helpers::installed_packages::get_all_installed_packages;
use crate::helpers::settings::{load_settings, save_settings};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn show_manage_favorites_dialog(parent: &gtk4::Window) {
    let all_packages = get_all_installed_packages();
    let settings = load_settings();

    let mut favorites: Vec<String> = all_packages
        .iter()
        .filter(|p| settings.favorite_packages.contains(p))
        .cloned()
        .collect();
    let mut others: Vec<String> = all_packages
        .iter()
        .filter(|p| !settings.favorite_packages.contains(p))
        .cloned()
        .collect();
    favorites.append(&mut others);
    let sorted_packages = favorites;

    let dialog = gtk4::Dialog::builder()
        .title("Manage Favorite Packages")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(520)
        .build();

    let content = dialog.content_area();
    content.set_spacing(0);
    content.set_vexpand(true);

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
        let is_fav = settings.favorite_packages.contains(pkg_name);

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

    dialog.add_button("Close", gtk4::ResponseType::Close);

    let checkboxes_clone = checkboxes.clone();
    dialog.connect_response(move |dlg, _| {
        let mut s = load_settings();
        s.favorite_packages = checkboxes_clone
            .borrow()
            .iter()
            .filter(|(_, cb)| cb.is_active())
            .map(|(name, _)| name.clone())
            .collect();
        if let Err(e) = save_settings(&s) {
            eprintln!("Failed to save favorite packages: {}", e);
        }
        dlg.close();
    });

    dialog.present();
}
