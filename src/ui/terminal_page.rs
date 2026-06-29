use glib::clone;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Frame, Label, Orientation};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use vte4::prelude::*;
use vte4::{Format, Terminal};

use crate::helpers::get_navigation_stack::get_navigation_stack;
use crate::helpers::settings::load_settings;
use crate::helpers::terminal::spawn_terminal;
use crate::helpers::tray_integration::trigger_check_service;
use crate::log_info;
use crate::ui::main_window::{POST_UPDATE_PAGE, load_packages};
use crate::ui::post_update_page::{reset_post_update_page, run_post_update_detections};

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
    title_label.set_widget_name("terminal-title");

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
    button_box.set_widget_name("terminal-button-box");

    let continue_btn = Button::with_label("Refresh Package List");
    continue_btn.add_css_class("suggested-action");

    button_box.append(&continue_btn);
    button_box.set_visible(false);

    main_box.append(&button_box);

    let command_finished = Arc::new(Mutex::new(false));
    let exit_code = Arc::new(Mutex::new(None));
    let last_exit: Arc<Mutex<i32>> = Arc::new(Mutex::new(-1));

    let last_exit_clone = last_exit.clone();
    let continue_btn_clone = continue_btn.clone();
    terminal.connect_child_exited(clone!(
        #[weak]
        button_box,
        #[weak]
        title_label,
        move |terminal, exit_status| {
            let mut finished = command_finished.lock().unwrap();
            *finished = true;

            let mut code = exit_code.lock().unwrap();
            *code = Some(exit_status);

            *last_exit_clone.lock().unwrap() = exit_status;

            log_info!("install terminal command exited: status={}", exit_status);
            capture_terminal_output(terminal, "install");

            if exit_status == 0 {
                title_label.set_text("Installation Completed Successfully");
                let settings = load_settings();
                if settings.run_post_update_checks {
                    continue_btn_clone.set_label("Continue to checks");
                } else {
                    continue_btn_clone.set_label("Refresh Package List");
                }
            } else {
                title_label.set_text("Installation Failed");
                continue_btn_clone.set_label("Refresh Package List");
                fire_update_failed_notification(exit_status);
            }

            button_box.set_visible(true);
        }
    ));

    let last_exit_btn = last_exit.clone();
    continue_btn.connect_clicked(clone!(
        #[weak]
        main_box,
        move |_| {
            log_info!("install terminal: continue button clicked");
            let exit = *last_exit_btn.lock().unwrap();
            let settings = load_settings();
            if exit == 0 {
                trigger_check_service();
            }
            if exit == 0 && settings.run_post_update_checks {
                go_to_post_update(&main_box);
            } else {
                refresh_package_list(&main_box);
            }
        }
    ));

    return main_box;
}

pub fn run_command_in_dialog<F>(parent: &gtk4::Window, command: &str, on_finished: F)
where
    F: Fn() + 'static,
{
    let dialog = gtk4::Window::builder()
        .title("Running command")
        .transient_for(parent)
        .modal(true)
        .default_width(820)
        .default_height(520)
        .build();

    let content_area = GtkBox::new(Orientation::Vertical, 0);
    content_area.set_vexpand(true);
    dialog.set_child(Some(&content_area));

    let header = GtkBox::new(Orientation::Vertical, 4);
    header.set_margin_start(12);
    header.set_margin_end(12);
    header.set_margin_top(12);
    header.set_margin_bottom(8);

    let title_label = Label::new(Some("Running command..."));
    title_label.add_css_class("title-3");
    title_label.set_xalign(0.0);
    header.append(&title_label);

    let subtitle_label = Label::new(Some(
        "Follow the prompts in the terminal below. The dialog will let you close once the command finishes.",
    ));
    subtitle_label.add_css_class("dim-label");
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    header.append(&subtitle_label);

    content_area.append(&header);

    let terminal = Terminal::new();
    terminal.set_vexpand(true);
    terminal.set_hexpand(true);
    terminal.set_scrollback_lines(1000);
    terminal.set_scroll_on_output(true);
    terminal.set_scroll_on_keystroke(true);
    terminal.set_audible_bell(false);

    let frame = Frame::new(Some("Terminal"));
    frame.set_child(Some(&terminal));
    frame.set_margin_start(12);
    frame.set_margin_end(12);
    frame.set_vexpand(true);
    content_area.append(&frame);

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_start(12);
    button_row.set_margin_end(12);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(12);

    let close_btn = Button::with_label("Close");
    close_btn.add_css_class("suggested-action");
    close_btn.set_sensitive(false);
    button_row.append(&close_btn);

    content_area.append(&button_row);

    let done_flag = Rc::new(RefCell::new(false));

    let title_for_exit = title_label.clone();
    let close_for_exit = close_btn.clone();
    let done_for_exit = done_flag.clone();
    terminal.connect_child_exited(move |term, exit_status| {
        *done_for_exit.borrow_mut() = true;
        log_info!("dialog terminal command exited: status={}", exit_status);
        capture_terminal_output(term, "dialog");
        if exit_status == 0 {
            title_for_exit.set_text("Command completed successfully");
        } else {
            title_for_exit.set_text(&format!("Command failed (exit {})", exit_status));
        }
        close_for_exit.set_sensitive(true);
    });

    let dialog_for_close_btn = dialog.clone();
    close_btn.connect_clicked(move |_| {
        dialog_for_close_btn.close();
    });

    let on_finished_rc: Rc<dyn Fn()> = Rc::new(on_finished);
    let done_for_request = done_flag.clone();
    let on_finished_for_request = on_finished_rc.clone();
    dialog.connect_close_request(move |_| {
        if !*done_for_request.borrow() {
            return glib::Propagation::Stop;
        }
        on_finished_for_request();
        return glib::Propagation::Proceed;
    });

    log_info!("spawning dialog terminal command: {}", command);
    dialog.present();
    spawn_terminal(&terminal, vec!["bash", "-lc", command]);
}

fn capture_terminal_output(terminal: &vte4::Terminal, label: &str) {
    let Some(text) = terminal.text_format(Format::Text) else {
        return;
    };
    let text_str: String = text.to_string();
    let trimmed = text_str.trim_end();
    if trimmed.is_empty() {
        return;
    }
    log_info!("terminal output ({}):\n{}", label, trimmed);
}

fn refresh_package_list(main_box: &GtkBox) {
    let Some((stack, content_box, window)) = get_navigation_stack(main_box) else {
        return;
    };

    stack.set_visible_child_name("loading");
    load_packages(stack, content_box, window);
}

fn go_to_post_update(main_box: &GtkBox) {
    let Some((stack, _content_box, window)) = get_navigation_stack(main_box) else {
        return;
    };

    if stack.child_by_name("post-update").is_none() {
        refresh_package_list(main_box);
        return;
    }

    POST_UPDATE_PAGE.with(|cell| {
        if let Some(page) = cell.borrow().as_ref() {
            reset_post_update_page(page);
        }
    });

    stack.set_visible_child_name("post-update");

    run_post_update_detections(window);
}

fn fire_update_failed_notification(exit_status: i32) {
    std::thread::spawn(move || {
        let result = notify_rust::Notification::new()
            .summary("Update Failed")
            .body(&format!(
                "The update command exited with status {}.",
                exit_status
            ))
            .icon("arch-update-manager")
            .appname("Arch Update Manager")
            .show();

        if let Err(e) = result {
            eprintln!("Failed to show update-failed notification: {}", e);
        }
    });
}
