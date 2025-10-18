use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation, Spinner};

pub fn create_loading_page() -> GtkBox {
    let loading_box = GtkBox::new(Orientation::Vertical, 20);
    loading_box.set_valign(gtk4::Align::Center);
    loading_box.set_halign(gtk4::Align::Center);

    let spinner = Spinner::new();
    spinner.set_spinning(true);
    spinner.set_width_request(48);
    spinner.set_height_request(48);

    let loading_label = Label::new(Some("Loading package updates..."));
    loading_label.add_css_class("title-3");

    loading_box.append(&spinner);
    loading_box.append(&loading_label);

    return loading_box;
}
