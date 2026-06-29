use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Label, Orientation, PolicyType, ScrolledWindow,
    Separator, Spinner,
};

use crate::helpers::elevated::open_url_as_user;
use crate::helpers::security::get_open_vulnerabilities;
use crate::log_info;
use crate::models::open_vulnerability::OpenVulnerability;
use crate::ui::dialogs::build_dialog_window;
use crate::ui::package_list::{prefers_dark, severity_color};

pub fn show_vulnerabilities_dialog(parent: &ApplicationWindow) {
    log_info!("vulnerabilities dialog opened");

    let (dialog, content_area) = build_dialog_window(parent, "Open Vulnerabilities", 620, 460);
    content_area.append(&build_loading_view());
    dialog.present();

    let content_for_async = content_area.clone();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(get_open_vulnerabilities).await;

        while let Some(child) = content_for_async.first_child() {
            content_for_async.remove(&child);
        }

        let body = match result {
            Ok(Some(vulnerabilities)) if vulnerabilities.is_empty() => {
                build_message_view("No unpatched vulnerabilities affect your installed packages.")
            }
            Ok(Some(vulnerabilities)) => build_list_view(&vulnerabilities),
            Ok(None) | Err(_) => build_message_view(
                "Could not load the Arch security tracker data. Check your connection and try again.",
            ),
        };
        content_for_async.append(&body);
    });
}

fn build_list_view(vulnerabilities: &[OpenVulnerability]) -> GtkBox {
    let dark = prefers_dark();

    let list_box = GtkBox::new(Orientation::Vertical, 16);
    list_box.set_margin_start(16);
    list_box.set_margin_end(16);
    list_box.set_margin_top(16);
    list_box.set_margin_bottom(16);

    let intro = Label::new(Some(&summary_text(vulnerabilities.len())));
    intro.set_xalign(0.0);
    intro.set_wrap(true);
    intro.add_css_class("dim-label");
    list_box.append(&intro);

    for vulnerability in vulnerabilities {
        list_box.append(&build_entry(vulnerability, dark));
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

fn build_entry(vulnerability: &OpenVulnerability, dark: bool) -> GtkBox {
    let entry = GtkBox::new(Orientation::Vertical, 4);

    let package_escaped = glib::markup_escape_text(&vulnerability.package);
    let package_url = format!(
        "https://security.archlinux.org/package/{}",
        vulnerability.package
    );
    let color = severity_color(&vulnerability.severity, dark);
    let severity_escaped = glib::markup_escape_text(&vulnerability.severity);

    let title = Label::new(None);
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_markup(&format!(
        "<a href=\"{}\"><b>{}</b></a>  <span foreground=\"{}\">[{}]</span>",
        glib::markup_escape_text(&package_url),
        package_escaped,
        color,
        severity_escaped
    ));
    title.connect_activate_link(|_, uri| {
        open_url_as_user(uri);
        return glib::Propagation::Stop;
    });
    entry.append(&title);

    if !vulnerability.types.is_empty() {
        let type_label = Label::new(Some(&vulnerability.types.join(", ")));
        type_label.set_xalign(0.0);
        type_label.set_wrap(true);
        type_label.add_css_class("dim-label");
        type_label.add_css_class("caption");
        entry.append(&type_label);
    }

    if !vulnerability.issues.is_empty() {
        let links = vulnerability
            .issues
            .iter()
            .map(|cve| {
                let escaped = glib::markup_escape_text(cve);
                format!(
                    "<a href=\"https://security.archlinux.org/{}\">{}</a>",
                    escaped, escaped
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let issues = Label::new(None);
        issues.set_xalign(0.0);
        issues.set_wrap(true);
        issues.set_margin_top(2);
        issues.set_markup(&links);
        issues.connect_activate_link(|_, uri| {
            open_url_as_user(uri);
            return glib::Propagation::Stop;
        });
        entry.append(&issues);
    }

    let separator = Separator::new(Orientation::Horizontal);
    separator.set_margin_top(8);
    entry.append(&separator);

    return entry;
}

fn summary_text(count: usize) -> String {
    if count == 1 {
        return "1 installed package has a known vulnerability with no fix available yet."
            .to_string();
    }
    return format!(
        "{} installed packages have known vulnerabilities with no fix available yet.",
        count
    );
}

fn build_loading_view() -> GtkBox {
    let wrapper = GtkBox::new(Orientation::Vertical, 12);
    wrapper.set_valign(Align::Center);
    wrapper.set_halign(Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let spinner = Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.start();
    wrapper.append(&spinner);

    let label = Label::new(Some("Checking the Arch security tracker..."));
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
