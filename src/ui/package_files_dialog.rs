use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Label, Orientation, PolicyType, ScrolledWindow, TextView, WrapMode};

use crate::helpers::package_files::list_package_files;
use crate::ui::dialogs::build_dialog_window;

pub fn show_package_files_dialog(parent: &ApplicationWindow, package: &str) {
    let (dialog, content) = build_dialog_window(parent, &format!("Files: {}", package), 720, 520);

    let body = match list_package_files(package) {
        Ok(files) if files.is_empty() => {
            build_message_view(&format!("No files found for '{}'.", package))
        }
        Ok(files) => build_file_list_view(&files),
        Err(e) => build_message_view(&format!("Failed to list files: {}", e)),
    };

    content.append(&body);
    dialog.present();
}

fn build_file_list_view(files: &[String]) -> gtk4::Box {
    let wrapper = gtk4::Box::new(Orientation::Vertical, 0);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let text_view = TextView::new();
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(WrapMode::None);
    text_view.set_monospace(true);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.buffer().set_text(&files.join("\n"));

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .child(&text_view)
        .build();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    wrapper.append(&scrolled);

    return wrapper;
}

fn build_message_view(message: &str) -> gtk4::Box {
    let wrapper = gtk4::Box::new(Orientation::Vertical, 12);
    wrapper.set_valign(gtk4::Align::Center);
    wrapper.set_halign(gtk4::Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let label = Label::new(Some(message));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    label.add_css_class("dim-label");
    wrapper.append(&label);

    return wrapper;
}
