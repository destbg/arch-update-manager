use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, Paned, ScrolledWindow, Window};
use similar::{ChangeTag, TextDiff};
use sourceview5::prelude::*;
use sourceview5::{Buffer, LanguageManager, StyleScheme, StyleSchemeManager, View};
use std::fs;
use std::path::Path;

use crate::log_info;

pub fn show_pacnew_diff_dialog(parent: &Window, pacnew_path: &str) {
    log_info!("pacnew dialog opened for {}", pacnew_path);
    let original_path = strip_pacnew_suffix(pacnew_path);

    let original_content = read_file_or_warn(&original_path);
    let pacnew_content = read_file_or_warn(pacnew_path);

    let dialog = Window::builder()
        .title(&format!("Review {}", pacnew_path))
        .transient_for(parent)
        .modal(true)
        .default_width(960)
        .default_height(620)
        .build();

    let content_area = GtkBox::new(Orientation::Vertical, 0);
    content_area.set_vexpand(true);
    dialog.set_child(Some(&content_area));

    let header_label = Label::new(Some(&format!(
        "Left side: {}\nRight side: {}",
        original_path, pacnew_path
    )));
    header_label.set_xalign(0.0);
    header_label.set_margin_start(12);
    header_label.set_margin_end(12);
    header_label.set_margin_top(8);
    header_label.set_margin_bottom(8);
    header_label.add_css_class("dim-label");
    content_area.append(&header_label);

    let language = guess_language(&original_path).or_else(|| guess_language(pacnew_path));

    let left_buffer = build_buffer(&original_content, language.as_ref());
    let right_buffer = build_buffer(&pacnew_content, language.as_ref());

    apply_diff_highlight(
        &left_buffer,
        &right_buffer,
        &original_content,
        &pacnew_content,
    );

    let left_view = build_source_view(&left_buffer, true);
    let right_view = build_source_view(&right_buffer, true);

    let paned = Paned::new(Orientation::Horizontal);
    paned.set_vexpand(true);
    paned.set_position(480);

    let left_scroll = wrap_in_scroll(&left_view, &original_path);
    let right_scroll = wrap_in_scroll(&right_view, pacnew_path);

    paned.set_start_child(Some(&left_scroll));
    paned.set_end_child(Some(&right_scroll));
    content_area.append(&paned);

    let button_row = GtkBox::new(Orientation::Horizontal, 8);
    button_row.set_margin_start(12);
    button_row.set_margin_end(12);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(8);
    button_row.set_halign(Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let keep_btn = Button::with_label("Keep current");
    let use_new_btn = Button::with_label("Use new");
    let save_btn = Button::with_label("Save merged");
    save_btn.add_css_class("suggested-action");

    button_row.append(&cancel_btn);
    button_row.append(&keep_btn);
    button_row.append(&use_new_btn);
    button_row.append(&save_btn);

    content_area.append(&button_row);

    let dialog_for_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        log_info!("pacnew dialog: Cancel clicked");
        dialog_for_cancel.close();
    });

    let dialog_for_keep = dialog.clone();
    let pacnew_for_keep = pacnew_path.to_string();
    keep_btn.connect_clicked(move |btn| {
        log_info!(
            "pacnew dialog: Keep current clicked for {}",
            pacnew_for_keep
        );
        let parent_window = btn.root().and_downcast::<Window>();
        match fs::remove_file(&pacnew_for_keep) {
            Ok(()) => dialog_for_keep.close(),
            Err(e) => show_error(
                parent_window.as_ref(),
                "Could not remove the pacnew file",
                &e.to_string(),
            ),
        }
    });

    let dialog_for_use = dialog.clone();
    let pacnew_for_use = pacnew_path.to_string();
    let original_for_use = original_path.clone();
    use_new_btn.connect_clicked(move |btn| {
        log_info!("pacnew dialog: Use new clicked for {}", pacnew_for_use);
        let parent_window = btn.root().and_downcast::<Window>();
        if let Err(e) = fs::copy(&pacnew_for_use, &original_for_use) {
            show_error(
                parent_window.as_ref(),
                "Could not replace the current file",
                &e.to_string(),
            );
            return;
        }
        if let Err(e) = fs::remove_file(&pacnew_for_use) {
            show_error(
                parent_window.as_ref(),
                "Could not remove the pacnew file",
                &e.to_string(),
            );
            return;
        }
        dialog_for_use.close();
    });

    let dialog_for_save = dialog.clone();
    let pacnew_for_save = pacnew_path.to_string();
    let original_for_save = original_path.clone();
    let left_buffer_for_save = left_buffer.clone();
    save_btn.connect_clicked(move |btn| {
        log_info!("pacnew dialog: Save merged clicked for {}", pacnew_for_save);
        let parent_window = btn.root().and_downcast::<Window>();
        let start = left_buffer_for_save.start_iter();
        let end = left_buffer_for_save.end_iter();
        let text = left_buffer_for_save.text(&start, &end, true).to_string();

        if let Err(e) = fs::write(&original_for_save, &text) {
            show_error(
                parent_window.as_ref(),
                "Could not save the merged file",
                &e.to_string(),
            );
            return;
        }
        if let Err(e) = fs::remove_file(&pacnew_for_save) {
            show_error(
                parent_window.as_ref(),
                "Could not remove the pacnew file",
                &e.to_string(),
            );
            return;
        }
        dialog_for_save.close();
    });

    dialog.present();
}

pub(crate) fn build_buffer(text: &str, language: Option<&sourceview5::Language>) -> Buffer {
    let buffer = Buffer::new(None);
    if let Some(lang) = language {
        buffer.set_language(Some(lang));
    }
    if let Some(scheme) = pick_style_scheme() {
        buffer.set_style_scheme(Some(&scheme));
    }
    buffer.set_highlight_syntax(true);
    buffer.set_text(text);
    return buffer;
}

pub(crate) fn build_source_view(buffer: &Buffer, editable: bool) -> View {
    let view = View::with_buffer(buffer);
    view.set_monospace(true);
    view.set_show_line_numbers(true);
    view.set_highlight_current_line(true);
    view.set_editable(editable);
    view.set_wrap_mode(gtk4::WrapMode::None);
    view.set_top_margin(6);
    view.set_bottom_margin(6);
    view.set_left_margin(6);
    view.set_right_margin(6);
    return view;
}

pub(crate) fn wrap_in_scroll(view: &View, file_label: &str) -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 0);

    let label = Label::new(Some(file_label));
    label.set_xalign(0.0);
    label.set_margin_start(8);
    label.set_margin_top(4);
    label.set_margin_bottom(4);
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    container.append(&label);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .child(view)
        .build();

    container.append(&scrolled);
    return container;
}

pub(crate) fn diff_highlight_colors() -> (&'static str, &'static str) {
    let prefer_dark = gtk4::Settings::default()
        .map(|s| s.is_gtk_application_prefer_dark_theme())
        .unwrap_or(false);

    if prefer_dark {
        return ("rgba(255, 110, 120, 0.22)", "rgba(90, 220, 130, 0.20)");
    }
    return ("rgba(220, 53, 69, 0.18)", "rgba(40, 167, 69, 0.18)");
}

fn strip_pacnew_suffix(path: &str) -> String {
    for suffix in [".pacnew", ".pacsave", ".pacorig"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    return path.to_string();
}

fn read_file_or_warn(path: &str) -> String {
    return match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not read {}: {}", path, e);
            String::new()
        }
    };
}

fn guess_language(path: &str) -> Option<sourceview5::Language> {
    let manager = LanguageManager::default();
    let p = Path::new(path);
    let file_name = p.file_name()?.to_str()?;
    let content_type = gio::content_type_guess(Some(file_name), None::<&[u8]>).0;
    return manager.guess_language(Some(file_name), Some(content_type.as_str()));
}

fn pick_style_scheme() -> Option<StyleScheme> {
    let manager = StyleSchemeManager::default();
    let prefer_dark = gtk4::Settings::default()
        .map(|s| s.is_gtk_application_prefer_dark_theme())
        .unwrap_or(false);

    let preferred_id = if prefer_dark {
        "Adwaita-dark"
    } else {
        "Adwaita"
    };

    if let Some(scheme) = manager.scheme(preferred_id) {
        return Some(scheme);
    }

    let fallback_id = if prefer_dark { "oblivion" } else { "classic" };
    return manager.scheme(fallback_id);
}

fn apply_diff_highlight(left: &Buffer, right: &Buffer, left_text: &str, right_text: &str) {
    let (removed_color, added_color) = diff_highlight_colors();
    let removed_tag = left.create_tag(
        Some("pacnew-removed"),
        &[("background", &removed_color.to_value())],
    );
    let added_tag = right.create_tag(
        Some("pacnew-added"),
        &[("background", &added_color.to_value())],
    );

    let diff = TextDiff::from_lines(left_text, right_text);

    let mut left_line: i32 = 0;
    let mut right_line: i32 = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                left_line += 1;
                right_line += 1;
            }
            ChangeTag::Delete => {
                if let Some(tag) = removed_tag.as_ref() {
                    if let (Some(start), Some(end)) = (
                        left.iter_at_line(left_line),
                        left.iter_at_line(left_line + 1),
                    ) {
                        left.apply_tag(tag, &start, &end);
                    }
                }
                left_line += 1;
            }
            ChangeTag::Insert => {
                if let Some(tag) = added_tag.as_ref() {
                    if let (Some(start), Some(end)) = (
                        right.iter_at_line(right_line),
                        right.iter_at_line(right_line + 1),
                    ) {
                        right.apply_tag(tag, &start, &end);
                    }
                }
                right_line += 1;
            }
        }
    }
}

fn show_error(parent: Option<&Window>, title: &str, message: &str) {
    let alert = gtk4::AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(message)
        .buttons(["OK"])
        .build();

    alert.show(parent);
}
