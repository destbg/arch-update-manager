use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Dialog, Label, Orientation, PolicyType, ResponseType, ScrolledWindow,
    Spinner, TextBuffer, TextView, Window, WrapMode,
};
use similar::{ChangeTag, TextDiff};
use sourceview5::LanguageManager;

use crate::helpers::aur_pkgbuild::prepare_pkgbuild_review;
use crate::log_info;
use crate::models::pkgbuild_review::PkgbuildReview;
use crate::ui::pacnew_diff::{
    build_buffer, build_source_view, diff_highlight_colors, wrap_in_scroll,
};

pub fn show_pkgbuild_review_dialog(parent: &Window, package: &str) {
    log_info!("pkgbuild review opened for {}", package);

    let dialog = Dialog::builder()
        .title(&format!("Review PKGBUILD: {}", package))
        .transient_for(parent)
        .modal(true)
        .default_width(820)
        .default_height(620)
        .build();

    let content_area = dialog.content_area();
    content_area.set_spacing(0);
    content_area.set_vexpand(true);
    content_area.set_hexpand(true);
    content_area.append(&build_loading_view(package));

    dialog.add_button("Close", ResponseType::Close);
    dialog.connect_response(|d, response| {
        if response == ResponseType::Close || response == ResponseType::DeleteEvent {
            d.close();
        }
    });

    dialog.present();

    let package_owned = package.to_string();
    let dialog_for_async = dialog.clone();
    glib::spawn_future_local(async move {
        let pkg = package_owned.clone();
        let result = gio::spawn_blocking(move || prepare_pkgbuild_review(&pkg)).await;

        let content_area = dialog_for_async.content_area();
        while let Some(child) = content_area.first_child() {
            content_area.remove(&child);
        }

        let body = match result {
            Ok(Ok(review)) => build_review_view(&review),
            Ok(Err(e)) => build_message_view(&format!("Failed to load PKGBUILD: {}", e)),
            Err(_) => build_message_view("Failed to load PKGBUILD (background task failed)."),
        };
        content_area.append(&body);
    });
}

fn build_review_view(review: &PkgbuildReview) -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.set_vexpand(true);
    container.set_hexpand(true);

    match &review.old_content {
        Some(old) if *old == review.new_content => {
            container.append(&header_label(&format!(
                "No changes - {} already matches {}.",
                review.old_label, review.new_label
            )));
            let buffer = build_buffer(&review.new_content, shell_language().as_ref());
            container.append(&wrap_in_scroll(
                &build_source_view(&buffer, false),
                &review.new_label,
            ));
        }
        Some(old) => {
            container.append(&header_label(
                "Differences between the locally cached PKGBUILD and the latest version from the AUR.",
            ));
            container.append(&build_inline_diff_view(old, &review.new_content));
        }
        None => {
            container.append(&header_label(
                "No locally cached PKGBUILD was found to compare against. Showing the latest version from the AUR.",
            ));
            let buffer = build_buffer(&review.new_content, shell_language().as_ref());
            container.append(&wrap_in_scroll(
                &build_source_view(&buffer, false),
                &review.new_label,
            ));
        }
    }

    return container;
}

fn build_inline_diff_view(old: &str, new: &str) -> ScrolledWindow {
    let diff = TextDiff::from_lines(old, new);

    let mut text = String::new();
    let mut changed_lines: Vec<(i32, bool)> = Vec::new();
    for (line_no, change) in diff.iter_all_changes().enumerate() {
        let line_no = line_no as i32;
        let value = change.value();
        let value = value.strip_suffix('\n').unwrap_or(value);
        let sign = match change.tag() {
            ChangeTag::Delete => {
                changed_lines.push((line_no, false));
                '-'
            }
            ChangeTag::Insert => {
                changed_lines.push((line_no, true));
                '+'
            }
            ChangeTag::Equal => ' ',
        };
        text.push(sign);
        text.push(' ');
        text.push_str(value);
        text.push('\n');
    }

    let buffer = TextBuffer::new(None);
    buffer.set_text(text.trim_end_matches('\n'));

    let (removed_color, added_color) = diff_highlight_colors();
    let removed = buffer.create_tag(Some("removed"), &[("background", &removed_color.to_value())]);
    let added = buffer.create_tag(Some("added"), &[("background", &added_color.to_value())]);

    for (line, is_addition) in changed_lines {
        let tag = if is_addition {
            added.as_ref()
        } else {
            removed.as_ref()
        };
        let (Some(tag), Some(start)) = (tag, buffer.iter_at_line(line)) else {
            continue;
        };
        let end = buffer
            .iter_at_line(line + 1)
            .unwrap_or_else(|| buffer.end_iter());
        buffer.apply_tag(tag, &start, &end);
    }

    let view = TextView::with_buffer(&buffer);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(WrapMode::None);
    view.set_left_margin(8);
    view.set_right_margin(8);
    view.set_top_margin(6);
    view.set_bottom_margin(6);

    return ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&view)
        .build();
}

fn header_label(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.set_margin_top(8);
    label.set_margin_bottom(8);
    label.add_css_class("dim-label");
    return label;
}

fn build_loading_view(package: &str) -> GtkBox {
    let wrapper = GtkBox::new(Orientation::Vertical, 12);
    wrapper.set_valign(Align::Center);
    wrapper.set_halign(Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let spinner = Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.start();
    wrapper.append(&spinner);

    let label = Label::new(Some(&format!("Fetching PKGBUILD for {}...", package)));
    label.add_css_class("dim-label");
    wrapper.append(&label);

    return wrapper;
}

fn build_message_view(message: &str) -> GtkBox {
    let wrapper = GtkBox::new(Orientation::Vertical, 12);
    wrapper.set_valign(Align::Center);
    wrapper.set_halign(Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let label = Label::new(Some(message));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    label.add_css_class("dim-label");
    wrapper.append(&label);

    return wrapper;
}

fn shell_language() -> Option<sourceview5::Language> {
    return LanguageManager::default().language("sh");
}
