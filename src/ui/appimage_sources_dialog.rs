use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, CheckButton, DropDown, Entry, Label, ListBox,
    ListBoxRow, Orientation, PolicyType, ScrolledWindow, Separator, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::helpers::appimage::{discover_appimages, embedded_source};
use crate::helpers::appimage_config::{
    import_shelly_sources, set_source_for_path, shelly_has_appimage_data, source_for_path,
};
use crate::helpers::aur::is_command_available;
use crate::log_info;
use crate::models::appimage_update_source::AppImageUpdateSource;
use crate::ui::dialogs::show_error_dialog;

pub fn show_appimage_sources_dialog(parent: &ApplicationWindow) {
    log_info!("appimage sources dialog opened");
    let dialog = Window::builder()
        .title("AppImage Update Sources")
        .transient_for(parent)
        .modal(true)
        .default_width(660)
        .default_height(540)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.set_vexpand(true);

    let top = GtkBox::new(Orientation::Horizontal, 8);
    top.set_margin_start(12);
    top.set_margin_end(12);
    top.set_margin_top(12);
    top.set_margin_bottom(8);

    let info = Label::new(Some(
        "Pick where each AppImage looks for new versions. AppImages with no source are not checked.",
    ));
    info.set_xalign(0.0);
    info.set_hexpand(true);
    info.set_wrap(true);
    info.add_css_class("dim-label");
    top.append(&info);

    let import_btn = Button::with_label("Import from shelly");
    import_btn.set_valign(Align::Start);
    import_btn.set_sensitive(is_command_available("shelly") && shelly_has_appimage_data());
    top.append(&import_btn);
    root.append(&top);

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&list)
        .build();
    scrolled.set_margin_start(12);
    scrolled.set_margin_end(12);
    root.append(&scrolled);

    root.append(&Separator::new(Orientation::Horizontal));

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_start(8);
    button_row.set_margin_end(8);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(8);
    let close_btn = Button::with_label("Close");
    let dialog_for_close = dialog.clone();
    close_btn.connect_clicked(move |_| dialog_for_close.close());
    button_row.append(&close_btn);
    root.append(&button_row);

    dialog.set_child(Some(&root));

    let populate: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let list_for_pop = list.clone();
    let dialog_for_pop = dialog.clone();
    let populate_inner = populate.clone();
    let pop_fn: Rc<dyn Fn()> = Rc::new(move || {
        while let Some(child) = list_for_pop.first_child() {
            list_for_pop.remove(&child);
        }
        let apps = discover_appimages();
        if apps.is_empty() {
            list_for_pop.append(&empty_row());
            return;
        }
        let refresh = populate_inner.borrow().clone();
        for app in apps {
            let row = build_row(&dialog_for_pop, &app.path, &app.name, refresh.clone());
            list_for_pop.append(&row);
        }
    });
    *populate.borrow_mut() = Some(pop_fn.clone());
    pop_fn();

    let pop_for_import = pop_fn.clone();
    let dialog_for_import = dialog.clone();
    import_btn.connect_clicked(move |_| match import_shelly_sources() {
        Ok((imported, skipped)) => {
            log_info!(
                "imported {} appimage sources from shelly, skipped {}",
                imported,
                skipped
            );
            pop_for_import();
        }
        Err(e) => {
            show_error_dialog(
                dialog_for_import.upcast_ref::<Window>(),
                "Could not import from shelly",
                &e.to_string(),
            );
        }
    });

    dialog.present();
}

fn empty_row() -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    let label = Label::new(Some(
        "No AppImages found in ~/.local/bin or ~/Applications.",
    ));
    label.set_margin_top(16);
    label.set_margin_bottom(16);
    label.add_css_class("dim-label");
    row.set_child(Some(&label));
    return row;
}

fn build_row(parent: &Window, path: &str, name: &str, refresh: Option<Rc<dyn Fn()>>) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);

    let hbox = GtkBox::new(Orientation::Horizontal, 12);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    let text = GtkBox::new(Orientation::Vertical, 2);
    text.set_hexpand(true);

    let title = Label::new(Some(name));
    title.set_xalign(0.0);
    title.add_css_class("heading");
    text.append(&title);

    let summary = Label::new(Some(&source_summary(path)));
    summary.set_xalign(0.0);
    summary.add_css_class("dim-label");
    summary.add_css_class("caption");
    text.append(&summary);

    hbox.append(&text);

    let edit_btn = Button::with_label("Set source");
    edit_btn.set_valign(Align::Center);
    let parent_for_edit = parent.clone();
    let path_for_edit = path.to_string();
    let name_for_edit = name.to_string();
    edit_btn.connect_clicked(move |_| {
        show_source_editor(
            &parent_for_edit,
            &path_for_edit,
            &name_for_edit,
            refresh.clone(),
        );
    });
    hbox.append(&edit_btn);

    row.set_child(Some(&hbox));
    return row;
}

fn source_summary(path: &str) -> String {
    if let Some(source) = source_for_path(path) {
        return format!("{} (set by you)", describe_source(&source));
    }
    let embedded = embedded_source(path);
    if !matches!(embedded, AppImageUpdateSource::None) {
        return format!("{} (from the AppImage)", describe_source(&embedded));
    }
    return "No update source".to_string();
}

fn describe_source(source: &AppImageUpdateSource) -> String {
    return match source {
        AppImageUpdateSource::None => "No update source".to_string(),
        AppImageUpdateSource::GitHub { owner, repo, .. } => {
            format!("GitHub releases: {}/{}", owner, repo)
        }
        AppImageUpdateSource::Zsync { url } => format!("zsync URL: {}", url),
    };
}

fn show_source_editor(parent: &Window, path: &str, name: &str, refresh: Option<Rc<dyn Fn()>>) {
    let editor = Window::builder()
        .title(format!("Update source: {}", name))
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);

    let body = GtkBox::new(Orientation::Vertical, 10);
    body.set_margin_start(16);
    body.set_margin_end(16);
    body.set_margin_top(16);
    body.set_margin_bottom(12);
    body.set_vexpand(true);

    let type_dropdown = DropDown::from_strings(&[
        "No automatic updates",
        "GitHub releases",
        "Static zsync URL",
    ]);
    body.append(&labeled("Update source", &type_dropdown));

    let github_box = GtkBox::new(Orientation::Vertical, 8);
    let github_entry = Entry::new();
    github_entry.set_placeholder_text(Some("owner/repo, for example FreeCAD/FreeCAD"));
    github_box.append(&labeled("GitHub repository", &github_entry));
    let prerelease_check = CheckButton::with_label("Include pre-releases");
    github_box.append(&prerelease_check);
    body.append(&github_box);

    let zsync_box = GtkBox::new(Orientation::Vertical, 8);
    let zsync_entry = Entry::new();
    zsync_entry.set_placeholder_text(Some("https://example.com/App-x86_64.AppImage.zsync"));
    zsync_box.append(&labeled("zsync file URL", &zsync_entry));
    body.append(&zsync_box);

    let current = source_for_path(path).unwrap_or_else(|| embedded_source(path));
    match &current {
        AppImageUpdateSource::None => type_dropdown.set_selected(0),
        AppImageUpdateSource::GitHub {
            owner,
            repo,
            prerelease,
        } => {
            type_dropdown.set_selected(1);
            github_entry.set_text(&format!("{}/{}", owner, repo));
            prerelease_check.set_active(*prerelease);
        }
        AppImageUpdateSource::Zsync { url } => {
            type_dropdown.set_selected(2);
            zsync_entry.set_text(url);
        }
    }

    let github_box_for_toggle = github_box.clone();
    let zsync_box_for_toggle = zsync_box.clone();
    let apply_visibility = move |dropdown: &DropDown| {
        let selected = dropdown.selected();
        github_box_for_toggle.set_visible(selected == 1);
        zsync_box_for_toggle.set_visible(selected == 2);
    };
    apply_visibility(&type_dropdown);
    type_dropdown.connect_selected_notify(move |dropdown| apply_visibility(dropdown));

    root.append(&body);
    root.append(&Separator::new(Orientation::Horizontal));

    let button_row = GtkBox::new(Orientation::Horizontal, 8);
    button_row.set_halign(Align::End);
    button_row.set_margin_start(8);
    button_row.set_margin_end(8);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(8);

    let cancel_btn = Button::with_label("Cancel");
    let editor_for_cancel = editor.clone();
    cancel_btn.connect_clicked(move |_| editor_for_cancel.close());
    button_row.append(&cancel_btn);

    let save_btn = Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    button_row.append(&save_btn);
    root.append(&button_row);

    editor.set_child(Some(&root));

    let editor_for_save = editor.clone();
    let path_for_save = path.to_string();
    let name_for_save = name.to_string();
    save_btn.connect_clicked(move |_| {
        let source = match type_dropdown.selected() {
            1 => {
                let raw = github_entry.text().to_string();
                let Some(source) = parse_owner_repo(&raw, prerelease_check.is_active()) else {
                    show_error_dialog(
                        editor_for_save.upcast_ref::<Window>(),
                        "Invalid repository",
                        "Enter the GitHub repository as owner/repo, for example FreeCAD/FreeCAD.",
                    );
                    return;
                };
                source
            }
            2 => {
                let url = zsync_entry.text().trim().to_string();
                if !url.starts_with("http") || !url.to_lowercase().ends_with(".zsync") {
                    show_error_dialog(
                        editor_for_save.upcast_ref::<Window>(),
                        "Invalid zsync URL",
                        "Enter the full URL of the .zsync file for this AppImage.",
                    );
                    return;
                }
                AppImageUpdateSource::Zsync { url }
            }
            _ => AppImageUpdateSource::None,
        };

        if let Err(e) = set_source_for_path(&path_for_save, &name_for_save, source) {
            show_error_dialog(
                editor_for_save.upcast_ref::<Window>(),
                "Could not save the update source",
                &e.to_string(),
            );
            return;
        }
        log_info!("saved appimage source for {}", path_for_save);
        editor_for_save.close();
        if let Some(refresh) = &refresh {
            refresh();
        }
    });

    editor.present();
}

fn parse_owner_repo(raw: &str, prerelease: bool) -> Option<AppImageUpdateSource> {
    let trimmed = raw.trim().trim_start_matches("https://github.com/");
    let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() != 2 {
        return None;
    }
    return Some(AppImageUpdateSource::GitHub {
        owner: parts[0].to_string(),
        repo: parts[1].to_string(),
        prerelease,
    });
}

fn labeled(label_text: &str, widget: &impl IsA<gtk4::Widget>) -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 4);
    let label = Label::new(Some(label_text));
    label.set_xalign(0.0);
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    container.append(&label);
    container.append(widget);
    return container;
}
