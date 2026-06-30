use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, CheckButton, DropDown, Entry, Expander,
    FileDialog, Label, ListBox, ListBoxRow, Orientation, Window, gio,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::helpers::appimage::{discover_appimages, embedded_source, managed_appimages};
use crate::helpers::appimage_config::{
    import_shelly_sources, remove_appimage_entry, set_source_for_path, shelly_has_appimage_data,
    source_for_path,
};
use crate::helpers::aur::is_command_available;
use crate::log_info;
use crate::models::appimage_update_source::AppImageUpdateSource;
use crate::ui::dialogs::show_error_dialog;

pub fn build_appimage_sources_section(parent: &ApplicationWindow) -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 8);

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    let add_btn = Button::with_label("Add AppImage…");
    actions.append(&add_btn);
    let import_btn = Button::with_label("Import from shelly");
    import_btn.set_sensitive(is_command_available("shelly") && shelly_has_appimage_data());
    actions.append(&import_btn);
    container.append(&actions);

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    container.append(&list);

    let populate_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let list_for_pop = list.clone();
    let parent_for_pop = parent.clone();
    let holder_for_pop = populate_holder.clone();
    let populate: Rc<dyn Fn()> = Rc::new(move || {
        while let Some(child) = list_for_pop.first_child() {
            list_for_pop.remove(&child);
        }
        let apps = managed_appimages();
        if apps.is_empty() {
            list_for_pop.append(&empty_row());
            return;
        }
        let discovered: HashSet<String> =
            discover_appimages().into_iter().map(|a| a.path).collect();
        let refresh = holder_for_pop.borrow().clone();
        for app in apps {
            let removable = !discovered.contains(&app.path);
            let row = build_row(
                &parent_for_pop,
                &app.path,
                &app.name,
                removable,
                refresh.clone(),
            );
            list_for_pop.append(&row);
        }
    });
    *populate_holder.borrow_mut() = Some(populate.clone());
    populate();

    let parent_for_add = parent.clone();
    let populate_for_add = populate.clone();
    add_btn.connect_clicked(move |_| {
        let dialog = FileDialog::builder().title("Add AppImage").build();
        let parent_err = parent_for_add.clone();
        let populate_cb = populate_for_add.clone();
        dialog.open(
            Some(&parent_for_add),
            gio::Cancellable::NONE,
            move |result| {
                let Ok(file) = result else {
                    return;
                };
                let Some(path) = file.path() else {
                    return;
                };
                let path_str = path.to_string_lossy().into_owned();
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("AppImage")
                    .to_string();
                let initial = embedded_source(&path_str);
                if let Err(e) = set_source_for_path(&path_str, &name, initial) {
                    show_error_dialog(
                        parent_err.upcast_ref::<Window>(),
                        "Could not add AppImage",
                        &e.to_string(),
                    );
                    return;
                }
                log_info!("added appimage {}", path_str);
                populate_cb();
            },
        );
    });

    let populate_for_import = populate.clone();
    let parent_for_import = parent.clone();
    import_btn.connect_clicked(move |_| match import_shelly_sources() {
        Ok((imported, skipped)) => {
            log_info!(
                "imported {} appimage sources from shelly, skipped {}",
                imported,
                skipped
            );
            populate_for_import();
        }
        Err(e) => {
            show_error_dialog(
                parent_for_import.upcast_ref::<Window>(),
                "Could not import from shelly",
                &e.to_string(),
            );
        }
    });

    return container;
}

fn empty_row() -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    let label = Label::new(Some(
        "No AppImages found. Drop one in ~/.local/bin or ~/Applications.",
    ));
    label.set_margin_top(16);
    label.set_margin_bottom(16);
    label.add_css_class("dim-label");
    row.set_child(Some(&label));
    return row;
}

fn build_row(
    parent: &ApplicationWindow,
    path: &str,
    name: &str,
    removable: bool,
    refresh: Option<Rc<dyn Fn()>>,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);

    let content = GtkBox::new(Orientation::Vertical, 8);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.set_margin_top(10);
    content.set_margin_bottom(10);

    let header = GtkBox::new(Orientation::Horizontal, 8);

    let titles = GtkBox::new(Orientation::Vertical, 2);
    titles.set_hexpand(true);

    let title = Label::new(Some(name));
    title.set_xalign(0.0);
    title.add_css_class("heading");
    titles.append(&title);

    let summary = Label::new(Some(&source_summary(path)));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    summary.set_max_width_chars(48);
    summary.add_css_class("dim-label");
    summary.add_css_class("caption");
    titles.append(&summary);

    header.append(&titles);

    if removable {
        let remove_btn = Button::with_label("Remove");
        remove_btn.set_valign(Align::Start);
        remove_btn.add_css_class("destructive-action");
        let path_for_remove = path.to_string();
        let refresh_for_remove = refresh.clone();
        remove_btn.connect_clicked(move |_| {
            if let Err(e) = remove_appimage_entry(&path_for_remove) {
                log_info!("failed to remove appimage {}: {}", path_for_remove, e);
                return;
            }
            log_info!("removed appimage {}", path_for_remove);
            if let Some(refresh) = &refresh_for_remove {
                refresh();
            }
        });
        header.append(&remove_btn);
    }

    let editor_expander = Expander::new(Some("Edit update source"));
    editor_expander.set_child(Some(&build_editor(parent, path, name, &summary)));

    content.append(&header);
    content.append(&editor_expander);

    row.set_child(Some(&content));
    return row;
}

fn build_editor(parent: &ApplicationWindow, path: &str, name: &str, summary: &Label) -> GtkBox {
    let body = GtkBox::new(Orientation::Vertical, 10);

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

    let save_btn = Button::with_label("Save source");
    save_btn.add_css_class("suggested-action");
    save_btn.set_halign(Align::Start);
    body.append(&save_btn);

    let parent_for_save = parent.clone();
    let path_for_save = path.to_string();
    let name_for_save = name.to_string();
    let summary_for_save = summary.clone();
    save_btn.connect_clicked(move |_| {
        let source = match type_dropdown.selected() {
            1 => {
                let raw = github_entry.text().to_string();
                let Some(source) = parse_owner_repo(&raw, prerelease_check.is_active()) else {
                    show_error_dialog(
                        parent_for_save.upcast_ref::<Window>(),
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
                        parent_for_save.upcast_ref::<Window>(),
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
                parent_for_save.upcast_ref::<Window>(),
                "Could not save the update source",
                &e.to_string(),
            );
            return;
        }
        log_info!("saved appimage source for {}", path_for_save);
        summary_for_save.set_text(&source_summary(&path_for_save));
    });

    return body;
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
