use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation};
use std::sync::{Arc, Mutex};
use vte4::{Terminal, TerminalExt};

use crate::helpers::get_navigation_stack::get_navigation_stack;

pub fn create_terminal_page() -> GtkBox {
    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_start(12);
    main_box.set_margin_end(12);
    main_box.set_margin_top(12);
    main_box.set_margin_bottom(12);

    let header_box = GtkBox::new(Orientation::Vertical, 6);

    let title_label = Label::new(Some("Installing Packages"));
    title_label.add_css_class("title-2");
    title_label.set_halign(gtk4::Align::Start);

    let subtitle_label = Label::new(Some(
        "Please follow the prompts in the terminal below to complete the installation",
    ));
    subtitle_label.add_css_class("dim-label");
    subtitle_label.set_halign(gtk4::Align::Start);

    header_box.append(&title_label);
    header_box.append(&subtitle_label);

    main_box.append(&header_box);

    let terminal = Terminal::new();
    terminal.set_vexpand(true);
    terminal.set_hexpand(true);
    terminal.set_scrollback_lines(-1);

    terminal.set_font_scale(1.0);
    terminal.set_audible_bell(false);
    terminal.set_scroll_on_output(true);
    terminal.set_scroll_on_keystroke(true);
    terminal.set_scrollback_lines(1000);

    let terminal_frame = gtk4::Frame::new(Some("Terminal"));
    terminal_frame.set_child(Some(&terminal));
    terminal_frame.set_vexpand(true);

    main_box.append(&terminal_frame);

    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::End);
    button_box.set_margin_top(12);

    let refresh_btn = Button::with_label("Refresh Package List");
    refresh_btn.add_css_class("suggested-action");

    button_box.append(&refresh_btn);
    button_box.set_visible(false);

    main_box.append(&button_box);

    let command_finished = Arc::new(Mutex::new(false));
    let exit_code = Arc::new(Mutex::new(None));

    terminal.connect_child_exited(clone!(
        #[weak]
        button_box,
        #[weak]
        title_label,
        move |_terminal, exit_status| {
            let mut finished = command_finished.lock().unwrap();
            *finished = true;

            let mut code = exit_code.lock().unwrap();
            *code = Some(exit_status);

            if exit_status == 0 {
                title_label.set_text("Installation Completed Successfully");
            } else {
                title_label.set_text("Installation Failed");
            }

            button_box.set_visible(true);
        }
    ));

    refresh_btn.connect_clicked(clone!(
        #[weak]
        main_box,
        move |_| {
            refresh_package_list(&main_box);
        }
    ));

    return main_box;
}

fn refresh_package_list(main_box: &GtkBox) {
    let Some((stack, content_box, window)) = get_navigation_stack(main_box) else {
        return;
    };

    stack.set_visible_child_name("loading");
    crate::ui::main_window::load_packages(stack, content_box, window);
}
