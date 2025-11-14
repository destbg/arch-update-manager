use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Orientation, ScrolledWindow, TextView};

use crate::helpers::database_lock::{is_lock_error, remove_database_lock};
use crate::helpers::get_navigation_stack::get_navigation_stack;

pub fn create_error_page() -> GtkBox {
    let error_box = GtkBox::new(Orientation::Vertical, 20);
    error_box.set_valign(gtk4::Align::Center);
    error_box.set_halign(gtk4::Align::Center);
    error_box.set_margin_start(40);
    error_box.set_margin_end(40);
    error_box.set_margin_top(40);
    error_box.set_margin_bottom(40);

    let icon = Image::from_icon_name("dialog-error-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("error");

    let title_label = Label::new(Some("Failed to Sync Package Databases"));
    title_label.add_css_class("title-2");

    let subtitle_label = Label::new(Some(
        "An error occurred while trying to synchronize package databases.",
    ));
    subtitle_label.add_css_class("dim-label");
    subtitle_label.set_wrap(true);
    subtitle_label.set_max_width_chars(60);

    let error_text_view = TextView::new();
    error_text_view.set_editable(false);
    error_text_view.set_cursor_visible(false);
    error_text_view.set_wrap_mode(gtk4::WrapMode::Word);
    error_text_view.set_monospace(true);
    error_text_view.set_margin_start(12);
    error_text_view.set_margin_end(12);
    error_text_view.set_margin_top(12);
    error_text_view.set_margin_bottom(12);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .min_content_width(500)
        .min_content_height(150)
        .max_content_height(300)
        .child(&error_text_view)
        .build();

    let error_frame = gtk4::Frame::new(Some("Error Details"));
    error_frame.set_child(Some(&scrolled));
    error_frame.set_margin_top(20);

    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Center);
    button_box.set_margin_top(20);

    let remove_lock_btn = Button::with_label("Remove Database Lock");
    remove_lock_btn.set_tooltip_text(Some("Remove /var/lib/pacman/db.lck file"));
    remove_lock_btn.set_visible(false);

    let retry_btn = Button::with_label("Try Again");
    retry_btn.add_css_class("suggested-action");

    remove_lock_btn.connect_clicked(clone!(
        #[weak]
        error_box,
        #[weak]
        remove_lock_btn,
        #[weak]
        retry_btn,
        move |_| {
            handle_remove_lock(&error_box, &remove_lock_btn, &retry_btn);
        }
    ));

    retry_btn.connect_clicked(clone!(
        #[weak]
        error_box,
        move |_| {
            handle_retry_click(&error_box);
        }
    ));

    button_box.append(&remove_lock_btn);
    button_box.append(&retry_btn);

    error_box.append(&icon);
    error_box.append(&title_label);
    error_box.append(&subtitle_label);
    error_box.append(&error_frame);
    error_box.append(&button_box);

    return error_box;
}

fn handle_retry_click(error_box: &GtkBox) {
    let Some((stack, content_box, window)) = get_navigation_stack(error_box) else {
        return;
    };

    stack.set_visible_child_name("loading");
    crate::ui::main_window::load_packages(stack, content_box, window);
}

fn handle_remove_lock(error_box: &GtkBox, remove_lock_btn: &Button, _retry_btn: &Button) {
    match remove_database_lock() {
        Ok(()) => {
            remove_lock_btn.set_visible(false);

            if let Some((stack, content_box, window)) = get_navigation_stack(error_box) {
                stack.set_visible_child_name("loading");
                crate::ui::main_window::load_packages(stack, content_box, window);
            }
        }
        Err(error_msg) => {
            if let Some((_, _, window)) = get_navigation_stack(error_box) {
                crate::ui::dialogs::show_error_dialog(
                    window.upcast_ref::<gtk4::Window>(),
                    "Failed to Remove Lock",
                    &error_msg,
                );
            }
        }
    }
}

pub fn update_error_page_message(error_box: &GtkBox, error_message: &str) {
    let mut child = error_box.first_child();
    for _ in 0..3 {
        if let Some(next) = child.as_ref().and_then(|c| c.next_sibling()) {
            child = Some(next);
        } else {
            return;
        }
    }

    if let Some(frame) = child.and_then(|c| c.downcast::<gtk4::Frame>().ok()) {
        if let Some(scrolled) = frame.child().and_downcast::<ScrolledWindow>() {
            if let Some(text_view) = scrolled.child().and_downcast::<TextView>() {
                let buffer = text_view.buffer();
                buffer.set_text(error_message);
            }
        }
    }

    let lock_error = is_lock_error(error_message);

    let mut child = error_box.first_child();
    for _ in 0..4 {
        if let Some(next) = child.as_ref().and_then(|c| c.next_sibling()) {
            child = Some(next);
        } else {
            return;
        }
    }

    if let Some(button_box) = child.and_downcast::<GtkBox>() {
        if let Some(remove_lock_btn) = button_box.first_child().and_downcast::<Button>() {
            remove_lock_btn.set_visible(lock_error);
        }
    }
}
