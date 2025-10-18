use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation, Separator};

pub fn create_info_panel() -> (GtkBox, Label) {
    let info_box = GtkBox::new(Orientation::Vertical, 6);
    info_box.set_margin_start(12);
    info_box.set_margin_end(12);
    info_box.set_margin_top(6);
    info_box.set_margin_bottom(6);

    let info_label = Label::new(Some("Information"));
    info_label.set_xalign(0.0);
    info_box.append(&info_label);

    let separator = Separator::new(Orientation::Horizontal);
    info_box.append(&separator);

    let info_text = Label::new(Some("Select a package to view its information."));
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

    info_box.append(&scrolled_window);

    return (info_box, info_text);
}
