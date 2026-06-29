use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow,
    SelectionMode,
};
use shlex::try_quote;
use std::cell::RefCell;
use std::rc::Rc;

use crate::helpers::pacman_cache::{list_cached_versions, package_path_to_string};
use crate::log_info;
use crate::models::cached_version::CachedVersion;
use crate::ui::context_menu::reload_package_list;
use crate::ui::dialogs::build_dialog_window;
use crate::ui::terminal_page::run_command_in_dialog;

pub fn show_downgrade_dialog(parent: &ApplicationWindow, package: &str, current_version: &str) {
    log_info!(
        "downgrade dialog opened for {} (current {})",
        package,
        current_version
    );
    let cached = list_cached_versions(package);
    let other_versions: Vec<CachedVersion> = cached
        .into_iter()
        .filter(|v| v.version != current_version)
        .collect();

    let (default_width, default_height) = if other_versions.is_empty() {
        (380, 220)
    } else {
        (560, 420)
    };

    if other_versions.is_empty() {
        let (dialog, content) = build_dialog_window(
            parent,
            &format!("Downgrade: {}", package),
            default_width,
            default_height,
        );
        content.append(&build_empty_body(package));
        dialog.present();
        return;
    }

    let dialog = gtk4::Window::builder()
        .title(&format!("Downgrade: {}", package))
        .transient_for(parent)
        .modal(true)
        .default_width(default_width)
        .default_height(default_height)
        .build();

    let root = gtk4::Box::new(Orientation::Vertical, 0);

    let body = gtk4::Box::new(Orientation::Vertical, 8);
    body.set_margin_start(16);
    body.set_margin_end(16);
    body.set_margin_top(12);
    body.set_margin_bottom(8);
    body.set_vexpand(true);

    let header = Label::new(Some(&format!(
        "Pick a cached version of '{}' to install. The current version is {}.",
        package, current_version
    )));
    header.set_wrap(true);
    header.set_xalign(0.0);
    header.add_css_class("dim-label");
    body.append(&header);

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::Single);
    list_box.add_css_class("boxed-list");

    for (idx, version) in other_versions.iter().enumerate() {
        let row = build_row(version);
        list_box.append(&row);
        if idx == 0 {
            list_box.select_row(Some(&row));
        }
    }

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .child(&list_box)
        .build();
    scrolled.set_vexpand(true);
    body.append(&scrolled);

    root.append(&body);
    root.append(&gtk4::Separator::new(Orientation::Horizontal));

    let button_row = gtk4::Box::new(Orientation::Horizontal, 8);
    button_row.set_halign(Align::End);
    button_row.set_margin_start(8);
    button_row.set_margin_end(8);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(8);

    let cancel_btn = gtk4::Button::with_label("Cancel");
    let install_btn = gtk4::Button::with_label("Install Selected");
    install_btn.add_css_class("suggested-action");
    button_row.append(&cancel_btn);
    button_row.append(&install_btn);
    root.append(&button_row);

    dialog.set_child(Some(&root));

    let dialog_for_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        log_info!("downgrade dialog dismissed");
        dialog_for_cancel.close();
    });

    let versions_for_response: Rc<RefCell<Vec<CachedVersion>>> =
        Rc::new(RefCell::new(other_versions));
    let parent_clone = parent.clone();
    let list_box_clone = list_box.clone();
    let package_for_response = package.to_string();
    let dialog_for_install = dialog.clone();
    install_btn.connect_clicked(move |_| {
        let Some(row) = list_box_clone.selected_row() else {
            dialog_for_install.close();
            return;
        };
        let idx = row.index();
        if idx < 0 {
            dialog_for_install.close();
            return;
        }
        let versions = versions_for_response.borrow();
        let Some(target) = versions.get(idx as usize).cloned() else {
            dialog_for_install.close();
            return;
        };
        drop(versions);

        let command = build_downgrade_command(&target);
        log_info!(
            "downgrade confirmed: {} -> {}",
            package_for_response,
            target.version
        );
        dialog_for_install.close();
        let window_for_reload = parent_clone.clone();
        run_command_in_dialog(
            parent_clone.upcast_ref::<gtk4::Window>(),
            &command,
            move || {
                reload_package_list(&window_for_reload);
            },
        );
    });

    dialog.present();
}

fn build_empty_body(package: &str) -> gtk4::Box {
    let wrapper = gtk4::Box::new(Orientation::Vertical, 12);
    wrapper.set_valign(Align::Center);
    wrapper.set_halign(Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);
    wrapper.set_margin_start(24);
    wrapper.set_margin_end(24);
    wrapper.set_margin_top(24);
    wrapper.set_margin_bottom(24);

    let label = Label::new(Some(&format!(
        "No older versions of '{}' were found in /var/cache/pacman/pkg/.\n\nThe pacman cache only contains packages you previously installed or that paccache has retained.",
        package
    )));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    label.add_css_class("dim-label");
    wrapper.append(&label);
    return wrapper;
}

fn build_row(version: &CachedVersion) -> ListBoxRow {
    let row = ListBoxRow::new();
    let vbox = gtk4::Box::new(Orientation::Vertical, 2);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(8);
    vbox.set_margin_bottom(8);

    let title = Label::new(Some(&version.version));
    title.set_xalign(0.0);
    title.add_css_class("heading");
    vbox.append(&title);

    let path_label = Label::new(Some(&package_path_to_string(&version.path)));
    path_label.set_xalign(0.0);
    path_label.set_ellipsize(gtk4::pango::EllipsizeMode::Start);
    path_label.add_css_class("dim-label");
    path_label.add_css_class("caption");
    vbox.append(&path_label);

    row.set_child(Some(&vbox));
    return row;
}

fn build_downgrade_command(target: &CachedVersion) -> String {
    let path = package_path_to_string(&target.path);
    let quoted = try_quote(&path)
        .map(|cow| cow.into_owned())
        .unwrap_or_else(|_| format!("'{}'", path.replace('\'', "'\\''")));
    return format!("sudo pacman -U --noconfirm {}", quoted);
}
