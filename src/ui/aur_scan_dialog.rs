use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Frame, Label, Orientation, PolicyType, ScrolledWindow, Separator,
    Spinner, Window, WrapMode,
};
use sourceview5::prelude::*;
use sourceview5::{LanguageManager, View};

use crate::helpers::aur_scan::scan_package;
use crate::log_info;
use crate::models::aur_scan_finding::AurScanFinding;
use crate::ui::dialogs::build_dialog_window;
use crate::ui::package_list::{prefers_dark, severity_color};
use crate::ui::pacnew_diff::build_buffer;

pub fn show_aur_scan_dialog(parent: &Window, package: &str) {
    log_info!("aur-scan results opened for {}", package);

    let (dialog, content_area) =
        build_dialog_window(parent, &format!("aur-scan: {}", package), 940, 560);
    content_area.append(&build_loading_view(package));
    dialog.present();

    let package_owned = package.to_string();
    let content_for_async = content_area.clone();
    glib::spawn_future_local(async move {
        let pkg = package_owned.clone();
        let result = gio::spawn_blocking(move || scan_package(&pkg)).await;

        while let Some(child) = content_for_async.first_child() {
            content_for_async.remove(&child);
        }

        let body = match result {
            Ok(findings) if findings.is_empty() => {
                build_message_view("aur-scan did not flag anything for this package.")
            }
            Ok(findings) => build_list_view(&findings),
            Err(_) => build_message_view("Could not run aur-scan for this package."),
        };
        content_for_async.append(&body);
    });
}

fn build_list_view(findings: &[AurScanFinding]) -> GtkBox {
    let dark = prefers_dark();

    let mut sorted: Vec<&AurScanFinding> = findings.iter().collect();
    sorted.sort_by(|a, b| b.severity_rank().cmp(&a.severity_rank()));

    let list_box = GtkBox::new(Orientation::Vertical, 16);
    list_box.set_margin_start(16);
    list_box.set_margin_end(16);
    list_box.set_margin_top(16);
    list_box.set_margin_bottom(16);

    for finding in sorted {
        list_box.append(&build_entry(finding, dark));
    }

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&list_box)
        .build();

    let wrapper = GtkBox::new(Orientation::Vertical, 0);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);
    wrapper.append(&scrolled);
    return wrapper;
}

fn build_entry(finding: &AurScanFinding, dark: bool) -> GtkBox {
    let entry = GtkBox::new(Orientation::Vertical, 4);

    let color = severity_color(&finding.severity, dark);
    let title = Label::new(None);
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_markup(&format!(
        "<b>{}</b>  <span foreground=\"{}\">[{}]</span>",
        glib::markup_escape_text(&finding.title),
        color,
        glib::markup_escape_text(&finding.severity),
    ));
    entry.append(&title);

    let meta = format!("{} \u{00B7} {}", finding.id, finding.category);
    let meta_label = Label::new(Some(&meta));
    meta_label.set_xalign(0.0);
    meta_label.set_wrap(true);
    meta_label.add_css_class("dim-label");
    meta_label.add_css_class("caption");
    entry.append(&meta_label);

    if !finding.description.is_empty() {
        let desc = Label::new(Some(&finding.description));
        desc.set_xalign(0.0);
        desc.set_wrap(true);
        desc.set_margin_top(2);
        entry.append(&desc);
    }

    if let Some(snippet) = &finding.snippet {
        if !snippet.trim().is_empty() {
            entry.append(&build_snippet(snippet, &finding.file));
        }
    }

    let separator = Separator::new(Orientation::Horizontal);
    separator.set_margin_top(8);
    entry.append(&separator);

    return entry;
}

fn build_snippet(snippet: &str, file: &Option<String>) -> Frame {
    let buffer = build_buffer(snippet.trim(), language_for(file).as_ref());

    let view = View::with_buffer(&buffer);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_show_line_numbers(false);
    view.set_hexpand(true);
    view.set_wrap_mode(WrapMode::None);
    view.set_left_margin(10);
    view.set_right_margin(10);
    view.set_top_margin(6);
    view.set_bottom_margin(6);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Never)
        .min_content_height(34)
        .child(&view)
        .build();
    scrolled.set_size_request(-1, 34);

    let frame = Frame::new(None);
    frame.set_margin_top(4);
    frame.set_child(Some(&scrolled));
    return frame;
}

fn language_for(file: &Option<String>) -> Option<sourceview5::Language> {
    let manager = LanguageManager::default();
    let name = file
        .as_deref()
        .and_then(|f| f.rsplit('/').next())
        .unwrap_or("PKGBUILD");

    if name == "PKGBUILD"
        || name.ends_with(".install")
        || name.ends_with(".sh")
        || name.ends_with(".bash")
    {
        return manager.language("sh");
    }

    let content_type = gio::content_type_guess(Some(name), None::<&[u8]>).0;
    return manager
        .guess_language(Some(name), Some(content_type.as_str()))
        .or_else(|| manager.language("sh"));
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

    let label = Label::new(Some(&format!("Scanning {} with aur-scan...", package)));
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
