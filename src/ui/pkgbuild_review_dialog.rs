use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Frame, Label, Orientation, PolicyType, ScrolledWindow, Spinner, Window,
    WrapMode,
};
use sourceview5::prelude::*;
use sourceview5::{LanguageManager, View};

use crate::helpers::aur_pkgbuild::prepare_pkgbuild_review;
use crate::log_info;
use crate::models::diff_row::DiffRow;
use crate::models::pkgbuild_review::PkgbuildReview;
use crate::ui::dialogs::build_dialog_window;
use crate::ui::package_list::prefers_dark;
use crate::ui::pacnew_diff::{
    build_buffer, build_source_view, diff_highlight_colors, wrap_in_scroll,
};

pub fn show_pkgbuild_review_dialog(parent: &Window, package: &str) {
    log_info!("pkgbuild review opened for {}", package);

    let (dialog, content_area) =
        build_dialog_window(parent, &format!("Review PKGBUILD: {}", package), 820, 620);
    content_area.append(&build_loading_view(package));
    dialog.present();

    let package_owned = package.to_string();
    let content_for_async = content_area.clone();
    glib::spawn_future_local(async move {
        let pkg = package_owned.clone();
        let result = gio::spawn_blocking(move || prepare_pkgbuild_review(&pkg)).await;

        while let Some(child) = content_for_async.first_child() {
            content_for_async.remove(&child);
        }

        let body = match result {
            Ok(Ok(review)) => build_review_view(&review),
            Ok(Err(e)) => build_message_view(&format!("Failed to load PKGBUILD: {}", e)),
            Err(_) => build_message_view("Failed to load PKGBUILD (background task failed)."),
        };
        content_for_async.append(&body);
    });
}

fn build_review_view(review: &PkgbuildReview) -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.set_vexpand(true);
    container.set_hexpand(true);

    if let Some(diff) = &review.diff {
        if diff.trim().is_empty() {
            container.append(&header_label(
                "No changes. Your local files already match the latest version from the AUR.",
            ));
            return container;
        }

        container.append(&header_label(
            "Changes to the package files since your installed version. Lines that were added or removed are the ones to look at.",
        ));
        container.append(&build_unified_diff_view(diff));
        return container;
    }

    if let Some(pkgbuild) = &review.pkgbuild {
        container.append(&header_label(
            "No local copy was found to compare against. Showing the latest PKGBUILD from the AUR.",
        ));
        let buffer = build_buffer(pkgbuild, shell_language().as_ref());
        container.append(&wrap_in_scroll(
            &build_source_view(&buffer, false),
            "AUR (latest)",
        ));
    }

    return container;
}

fn parse_diff(diff: &str) -> Vec<DiffRow> {
    let mut rows: Vec<DiffRow> = Vec::new();
    let mut last_file: Option<usize> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            rows.push(DiffRow::File {
                path: diff_file_path(rest),
                change: "Modified",
            });
            last_file = Some(rows.len() - 1);
        } else if line.starts_with("new file") {
            set_file_change(&mut rows, last_file, "Added");
        } else if line.starts_with("deleted file") {
            set_file_change(&mut rows, last_file, "Deleted");
        } else if line.starts_with("rename ") {
            set_file_change(&mut rows, last_file, "Renamed");
        } else if line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("similarity ")
            || line.starts_with('\\')
        {
        } else if line.starts_with("@@") {
            rows.push(DiffRow::Hunk {
                context: hunk_context(line),
            });
        } else if let Some(rest) = line.strip_prefix('+') {
            rows.push(DiffRow::Added(rest.to_string()));
        } else if let Some(rest) = line.strip_prefix('-') {
            rows.push(DiffRow::Removed(rest.to_string()));
        } else {
            rows.push(DiffRow::Context(
                line.strip_prefix(' ').unwrap_or(line).to_string(),
            ));
        }
    }

    return rows;
}

fn set_file_change(rows: &mut [DiffRow], idx: Option<usize>, change: &'static str) {
    if let Some(DiffRow::File { change: c, .. }) = idx.and_then(|i| rows.get_mut(i)) {
        *c = change;
    }
}

fn diff_file_path(rest: &str) -> String {
    if let Some(pos) = rest.rfind(" b/") {
        return rest[pos + 3..].to_string();
    }
    return rest
        .split_whitespace()
        .next()
        .map(|s| s.trim_start_matches("a/").to_string())
        .unwrap_or_default();
}

fn hunk_context(line: &str) -> String {
    if let Some(pos) = line.rfind("@@") {
        return line[pos + 2..].trim().to_string();
    }
    return String::new();
}

fn build_unified_diff_view(diff: &str) -> ScrolledWindow {
    let rows = parse_diff(diff);

    let mut groups: Vec<(String, &'static str, Vec<&DiffRow>)> = Vec::new();
    for row in &rows {
        if let DiffRow::File { path, change } = row {
            groups.push((path.clone(), change, Vec::new()));
        } else if let Some(last) = groups.last_mut() {
            last.2.push(row);
        }
    }

    let list = GtkBox::new(Orientation::Vertical, 12);
    list.set_margin_start(12);
    list.set_margin_end(12);
    list.set_margin_top(8);
    list.set_margin_bottom(12);

    for (path, change, content) in &groups {
        list.append(&build_file_card(path, change, content));
    }

    return ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&list)
        .build();
}

fn build_file_card(path: &str, change: &str, rows: &[&DiffRow]) -> Frame {
    let dark = prefers_dark();

    let header = Label::new(None);
    header.set_xalign(0.0);
    header.set_margin_start(4);
    header.set_margin_end(4);
    let change_color = match change {
        "Added" => {
            if dark {
                "#5adc82"
            } else {
                "#2a9d4a"
            }
        }
        "Deleted" => {
            if dark {
                "#ff6b6b"
            } else {
                "#c0143c"
            }
        }
        "Renamed" => {
            if dark {
                "#ffa348"
            } else {
                "#e66100"
            }
        }
        _ => {
            if dark {
                "#a0a0a0"
            } else {
                "#6a6a6a"
            }
        }
    };
    header.set_markup(&format!(
        "<b>{}</b>   <span foreground=\"{}\">{}</span>",
        glib::markup_escape_text(path),
        change_color,
        change
    ));

    let content = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Never)
        .propagate_natural_height(true)
        .child(&build_file_diff_view(path, rows))
        .build();

    let frame = Frame::new(None);
    frame.set_label_widget(Some(&header));
    frame.set_label_align(0.0);
    frame.set_child(Some(&content));
    return frame;
}

fn build_file_diff_view(path: &str, rows: &[&DiffRow]) -> View {
    let mut text = String::new();
    let mut kinds: Vec<&'static str> = Vec::new();

    for &row in rows {
        let (line, kind) = match row {
            DiffRow::Context(t) => (t.as_str(), ""),
            DiffRow::Added(t) => (t.as_str(), "added"),
            DiffRow::Removed(t) => (t.as_str(), "removed"),
            _ => continue,
        };
        text.push_str(line);
        text.push('\n');
        kinds.push(kind);
    }

    let buffer = build_buffer(
        text.trim_end_matches('\n'),
        language_for_file(path).as_ref(),
    );

    let (removed_color, added_color) = diff_highlight_colors();
    let added = buffer.create_tag(
        Some("added"),
        &[("paragraph-background", &added_color.to_value())],
    );
    let removed = buffer.create_tag(
        Some("removed"),
        &[("paragraph-background", &removed_color.to_value())],
    );

    for (line_no, kind) in kinds.iter().enumerate() {
        let tag = match *kind {
            "added" => added.as_ref(),
            "removed" => removed.as_ref(),
            _ => None,
        };
        let (Some(tag), Some(start)) = (tag, buffer.iter_at_line(line_no as i32)) else {
            continue;
        };
        let end = buffer
            .iter_at_line(line_no as i32 + 1)
            .unwrap_or_else(|| buffer.end_iter());
        buffer.apply_tag(tag, &start, &end);
    }

    let view = View::with_buffer(&buffer);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_show_line_numbers(false);
    view.set_wrap_mode(WrapMode::None);
    view.set_left_margin(10);
    view.set_right_margin(10);
    view.set_top_margin(8);
    view.set_bottom_margin(8);
    return view;
}

fn language_for_file(path: &str) -> Option<sourceview5::Language> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let manager = LanguageManager::default();

    let explicit = if name == "PKGBUILD"
        || name.ends_with(".install")
        || name.ends_with(".sh")
        || name.ends_with(".bash")
    {
        Some("sh")
    } else if name.ends_with(".patch") || name.ends_with(".diff") {
        Some("diff")
    } else if name.ends_with(".service")
        || name.ends_with(".timer")
        || name.ends_with(".socket")
        || name.ends_with(".conf")
        || name.ends_with(".cfg")
        || name.ends_with(".ini")
    {
        Some("ini")
    } else if name.ends_with(".desktop") {
        Some("desktop")
    } else {
        None
    };

    if let Some(id) = explicit {
        if let Some(lang) = manager.language(id) {
            return Some(lang);
        }
    }

    let content_type = gio::content_type_guess(Some(name), None::<&[u8]>).0;
    return manager.guess_language(Some(name), Some(content_type.as_str()));
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
