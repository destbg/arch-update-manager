use gtk4::prelude::*;
use gtk4::{
    AlertDialog, Align, ApplicationWindow, Box as GtkBox, Button, Label, Orientation, Separator,
    Spinner, Window,
};

pub fn build_dialog_window(
    parent: &impl IsA<Window>,
    title: &str,
    width: i32,
    height: i32,
) -> (Window, GtkBox) {
    let window = Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(width)
        .default_height(height)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_vexpand(true);
    content.set_hexpand(true);
    root.append(&content);

    root.append(&Separator::new(Orientation::Horizontal));

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_start(8);
    button_row.set_margin_end(8);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(8);

    let close_button = Button::with_label("Close");
    let window_for_close = window.clone();
    close_button.connect_clicked(move |_| window_for_close.close());
    button_row.append(&close_button);
    root.append(&button_row);

    window.set_child(Some(&root));
    return (window, content);
}

pub fn show_error_dialog(parent: &Window, title: &str, message: &str) {
    let alert = AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(message)
        .buttons(["OK"])
        .build();

    alert.show(Some(parent));
}

pub fn create_progress_dialog(parent: &Window, title: &str, message: &str) -> Window {
    let window = Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .resizable(false)
        .build();

    let content = GtkBox::new(Orientation::Horizontal, 12);
    content.set_margin_start(20);
    content.set_margin_end(20);
    content.set_margin_top(20);
    content.set_margin_bottom(20);

    let spinner = Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.set_valign(gtk4::Align::Center);
    spinner.start();
    content.append(&spinner);

    let label = Label::new(Some(message));
    label.set_wrap(true);
    label.set_xalign(0.0);
    content.append(&label);

    window.set_child(Some(&content));
    window.present();

    return window;
}

pub fn show_confirm_dialog(
    parent: &ApplicationWindow,
    title: &str,
    message: &str,
    accept_label: &str,
    on_result: impl FnOnce(bool) + 'static,
) {
    let alert = AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(message)
        .buttons(["Cancel", accept_label])
        .cancel_button(0)
        .default_button(1)
        .build();

    alert.choose(Some(parent), gio::Cancellable::NONE, move |result| {
        on_result(matches!(result, Ok(1)));
    });
}

pub fn show_partial_upgrade_dialog(
    parent: &ApplicationWindow,
    message: &str,
    on_full_upgrade: impl FnOnce() + 'static,
    on_install_selected: impl FnOnce() + 'static,
) {
    let alert = AlertDialog::builder()
        .modal(true)
        .message("Partial upgrade")
        .detail(message)
        .buttons(["Cancel", "Install selected anyway", "Full upgrade"])
        .cancel_button(0)
        .default_button(2)
        .build();

    alert.choose(
        Some(parent),
        gio::Cancellable::NONE,
        move |result| match result {
            Ok(2) => on_full_upgrade(),
            Ok(1) => on_install_selected(),
            _ => {}
        },
    );
}
