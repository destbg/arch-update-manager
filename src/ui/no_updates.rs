use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Orientation};

use crate::helpers::get_navigation_stack::get_navigation_stack;

pub fn create_no_updates_page() -> GtkBox {
    let no_updates_box = GtkBox::new(Orientation::Vertical, 20);
    no_updates_box.set_valign(gtk4::Align::Center);
    no_updates_box.set_halign(gtk4::Align::Center);

    let icon = Image::from_icon_name("object-select-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("success");

    let title_label = Label::new(Some("System is up to date"));
    title_label.add_css_class("title-2");

    let subtitle_label = Label::new(Some("All packages are already at their latest versions"));
    subtitle_label.add_css_class("dim-label");

    let refresh_btn = Button::with_label("Check for Updates");
    refresh_btn.add_css_class("suggested-action");
    refresh_btn.set_margin_top(20);

    refresh_btn.connect_clicked(clone!(
        #[weak]
        no_updates_box,
        move |_| {
            handle_refresh_click(&no_updates_box);
        }
    ));

    no_updates_box.append(&icon);
    no_updates_box.append(&title_label);
    no_updates_box.append(&subtitle_label);
    no_updates_box.append(&refresh_btn);

    return no_updates_box;
}

fn handle_refresh_click(no_updates_box: &GtkBox) {
    let Some((stack, content_box, window)) = get_navigation_stack(no_updates_box) else {
        return;
    };

    stack.set_visible_child_name("loading");
    crate::ui::main_window::load_packages(stack, content_box, window);
}
