use gtk4::{ApplicationWindow, ResponseType, prelude::*};
use gtk4::{Box as GtkBox, ButtonsType, Dialog, MessageDialog, MessageType, Spinner, Window};

pub fn show_error_dialog(parent: &Window, title: &str, message: &str) {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(MessageType::Error)
        .buttons(ButtonsType::Ok)
        .text(title)
        .secondary_text(message)
        .build();

    dialog.connect_response(|dialog, _| {
        dialog.close();
    });

    dialog.show();
}

pub fn create_progress_dialog(parent: &Window, title: &str, message: &str) -> Dialog {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(MessageType::Info)
        .buttons(ButtonsType::Cancel)
        .text(title)
        .secondary_text(message)
        .build();

    let content_area = dialog.message_area();
    let spinner = Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.set_margin_end(12);
    spinner.start();

    if let Some(box_area) = content_area.downcast_ref::<GtkBox>() {
        box_area.prepend(&spinner);
    }

    dialog.connect_response(|dialog, _| {
        dialog.close();
    });

    dialog.show();

    return dialog.upcast::<Dialog>();
}

pub fn show_confirm_dialog(
    parent: &ApplicationWindow,
    title: &str,
    message: &str,
    accept_label: &str,
) -> MessageDialog {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(MessageType::Question)
        .text(title)
        .secondary_text(message)
        .build();

    dialog.add_button("Cancel", ResponseType::Cancel);
    dialog.add_button(accept_label, ResponseType::Accept);

    dialog.show();

    return dialog;
}

pub fn show_partial_upgrade_dialog(parent: &ApplicationWindow, message: &str) -> MessageDialog {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(MessageType::Warning)
        .text("Partial upgrade")
        .secondary_text(message)
        .build();

    dialog.add_button("Cancel", ResponseType::Cancel);
    dialog.add_button("Install selected anyway", ResponseType::Accept);
    dialog.add_button("Full upgrade", ResponseType::Yes);

    dialog.show();

    return dialog;
}
