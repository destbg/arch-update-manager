use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, Separator};
use std::cell::RefCell;
use std::rc::Rc;

use crate::helpers::elevated::open_url_as_user;
use crate::models::info_panel::InfoPanel;

pub fn create_info_panel() -> InfoPanel {
    let info_box = GtkBox::new(Orientation::Vertical, 6);
    info_box.set_margin_start(12);
    info_box.set_margin_end(12);
    info_box.set_margin_top(6);
    info_box.set_margin_bottom(6);

    let header = GtkBox::new(Orientation::Horizontal, 6);

    let info_label = Label::new(Some("Information"));
    info_label.set_xalign(0.0);
    info_label.set_hexpand(true);
    header.append(&info_label);

    let url_button = Button::from_icon_name("web-browser-symbolic");
    url_button.set_tooltip_text(Some("Open homepage"));
    url_button.add_css_class("flat");
    url_button.set_halign(Align::End);
    url_button.set_visible(false);
    header.append(&url_button);

    info_box.append(&header);

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

    let current_url: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let current_url_clone = current_url.clone();
    url_button.connect_clicked(move |_| {
        if let Some(url) = current_url_clone.borrow().clone() {
            open_url_as_user(&url);
        }
    });

    return InfoPanel {
        container: info_box,
        info_text,
        url_button,
        current_url,
    };
}
