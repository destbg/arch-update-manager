use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::repo_switch::{RepoSwitch, SwitchKind};

pub fn show_repo_switch_dialog(
    parent: &gtk4::Window,
    switches: Rc<RefCell<Vec<RepoSwitch>>>,
    on_apply: impl Fn() + 'static,
) {
    let dialog = gtk4::Dialog::builder()
        .title("Repository Switches")
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .default_height(420)
        .build();

    let content = dialog.content_area();
    content.set_spacing(0);
    content.set_vexpand(true);

    let header = gtk4::Label::new(Some(
        "Tick the switches you want to apply. They'll be included in the next Install Updates action.",
    ));
    header.set_wrap(true);
    header.set_xalign(0.0);
    header.set_margin_start(16);
    header.set_margin_end(16);
    header.set_margin_top(16);
    header.set_margin_bottom(8);
    header.add_css_class("dim-label");
    content.append(&header);

    let scrolled = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);
    list_box.set_margin_bottom(12);

    for (index, switch) in switches.borrow().iter().enumerate() {
        list_box.append(&build_row(index, switch, switches.clone()));
    }

    scrolled.set_child(Some(&list_box));
    content.append(&scrolled);

    dialog.add_button("Close", gtk4::ResponseType::Close);

    dialog.connect_response(move |dlg, _| {
        on_apply();
        dlg.close();
    });

    dialog.present();
}

fn build_row(
    index: usize,
    switch: &RepoSwitch,
    switches: Rc<RefCell<Vec<RepoSwitch>>>,
) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    let check = gtk4::CheckButton::new();
    check.set_active(switch.selected);
    check.set_valign(gtk4::Align::Center);
    {
        let switches = switches.clone();
        check.connect_toggled(move |c| {
            if let Some(entry) = switches.borrow_mut().get_mut(index) {
                entry.selected = c.is_active();
            }
        });
    }
    hbox.append(&check);

    let labels = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    labels.set_hexpand(true);

    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    title.set_halign(gtk4::Align::Start);
    title.set_use_markup(true);
    title.set_markup(&format_title(switch));
    labels.append(&title);

    let subtitle = gtk4::Label::new(Some(&format_subtitle(switch)));
    subtitle.set_xalign(0.0);
    subtitle.set_halign(gtk4::Align::Start);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    labels.append(&subtitle);

    hbox.append(&labels);
    row.set_child(Some(&hbox));

    return row;
}

fn format_title(switch: &RepoSwitch) -> String {
    return match switch.kind {
        SwitchKind::RepoChange => format!(
            "<b>{}</b>: {} -> {}",
            escape(&switch.installed_name),
            escape(&switch.installed_repo),
            escape(&switch.target_repo),
        ),
        SwitchKind::Replace => format!(
            "<b>{}</b> -> <b>{}/{}</b>",
            escape(&switch.installed_name),
            escape(&switch.target_repo),
            escape(&switch.target_name),
        ),
    };
}

fn format_subtitle(switch: &RepoSwitch) -> String {
    return match switch.kind {
        SwitchKind::RepoChange => {
            if switch.installed_version == switch.target_version {
                format!("version {} (same)", switch.installed_version)
            } else {
                format!(
                    "version {} -> {}",
                    switch.installed_version, switch.target_version
                )
            }
        }
        SwitchKind::Replace => format!(
            "replaces {} ({} -> {})",
            switch.installed_name, switch.installed_version, switch.target_version
        ),
    };
}

fn escape(s: &str) -> String {
    return glib::markup_escape_text(s).to_string();
}
