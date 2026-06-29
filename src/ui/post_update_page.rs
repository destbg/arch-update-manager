use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, CheckButton, Image, Label, ListBox,
    ListBoxRow, ListItem, ListView, NoSelection, Orientation, ScrolledWindow, SelectionMode,
    Separator, SignalListItemFactory, Spinner, StringList, StringObject,
};
use shlex::try_quote;
use std::cell::RefCell;
use std::rc::Rc;

use crate::helpers::elevated::spawn_as_user_or_root;
use crate::helpers::flatpak::{build_flatpak_uninstall_command, get_unused_flatpak_runtimes};
use crate::helpers::post_update::{
    clean_cache, get_cache_candidates, get_orphan_packages, get_pacnew_files,
    get_services_needing_restart, is_kernel_reboot_pending, is_meld_available, restart_service,
};
use crate::helpers::repo_switches::detect_repo_switches;
use crate::helpers::settings::load_settings;
use crate::log_info;
use crate::models::cache_candidates::CacheCandidates;
use crate::models::flatpak_installation::FlatpakInstallation;
use crate::models::post_update_page::PostUpdatePage;
use crate::models::repo_switch::{RepoSwitch, SwitchKind};
use crate::models::section::Section;
use crate::models::section_visibility::SectionVisibility;
use crate::models::service_restart_outcome::ServiceRestartOutcome;
use crate::models::service_row_state::ServiceRowState;
use crate::ui::main_window::POST_UPDATE_PAGE;
use crate::ui::pacnew_diff::show_pacnew_diff_dialog;
use crate::ui::terminal_page::run_command_in_dialog;

pub fn create_post_update_page() -> PostUpdatePage {
    let container = GtkBox::new(Orientation::Vertical, 0);

    let reboot_banner = build_reboot_banner();
    container.append(&reboot_banner);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let content = GtkBox::new(Orientation::Vertical, 12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);

    let header_box = GtkBox::new(Orientation::Vertical, 6);

    let header_label = Label::new(Some("Post-update checks"));
    header_label.add_css_class("title-2");
    header_label.set_halign(Align::Start);

    let header_caption = Label::new(Some(
        "Review and apply any maintenance steps after the install.",
    ));
    header_caption.add_css_class("dim-label");
    header_caption.set_halign(Align::Start);
    header_caption.set_wrap(true);

    header_box.append(&header_label);
    header_box.append(&header_caption);
    content.append(&header_box);

    let header_separator = Separator::new(Orientation::Horizontal);
    content.append(&header_separator);

    let loading_box = build_loading_box();
    content.append(&loading_box);

    let all_clear_box = build_all_clear_box();
    content.append(&all_clear_box);

    let sections_box = GtkBox::new(Orientation::Vertical, 18);
    content.append(&sections_box);

    scrolled.set_child(Some(&content));
    container.append(&scrolled);

    let bottom_bar = GtkBox::new(Orientation::Horizontal, 8);
    bottom_bar.set_margin_start(12);
    bottom_bar.set_margin_end(12);
    bottom_bar.set_margin_top(12);
    bottom_bar.set_margin_bottom(12);

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bottom_bar.append(&spacer);

    let back_button = Button::with_label("Back to packages");
    back_button.add_css_class("suggested-action");
    bottom_bar.append(&back_button);

    container.append(&bottom_bar);

    return PostUpdatePage {
        container,
        reboot_banner,
        loading_box,
        all_clear_box,
        sections_box,
        back_button,
        section_visibility: Rc::new(RefCell::new(SectionVisibility::default())),
    };
}

pub fn reset_post_update_page(page: &PostUpdatePage) {
    page.reboot_banner.set_visible(false);

    let mut child = page.sections_box.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        page.sections_box.remove(&widget);
        child = next;
    }

    *page.section_visibility.borrow_mut() = SectionVisibility::default();
    page.all_clear_box.set_visible(false);
    page.loading_box.set_visible(true);
    start_loading_spinner(&page.loading_box);
}

pub fn finish_post_update_loading(page: &PostUpdatePage) {
    stop_loading_spinner(&page.loading_box);
    page.loading_box.set_visible(false);
    refresh_all_clear(page);
}

pub fn refresh_all_clear(page: &PostUpdatePage) {
    let visibility = page.section_visibility.borrow();
    let any_visible = visibility.orphans
        || visibility.cache
        || visibility.pacnew
        || visibility.services
        || visibility.flatpak_unused
        || visibility.resolutions;
    page.all_clear_box.set_visible(!any_visible);
}

pub fn build_section_box(title: &str) -> Section {
    install_post_update_css();

    let wrapper = GtkBox::new(Orientation::Vertical, 8);
    wrapper.add_css_class("post-update-section");

    let header = Label::new(Some(title));
    header.add_css_class("heading");
    header.set_xalign(0.0);
    header.set_margin_start(4);
    wrapper.append(&header);

    let card = GtkBox::new(Orientation::Vertical, 10);
    card.add_css_class("post-update-card");
    wrapper.append(&card);

    return Section { wrapper, card };
}

pub fn set_orphans_section(
    page: &PostUpdatePage,
    packages: Vec<String>,
    window: &ApplicationWindow,
) {
    if packages.is_empty() {
        return;
    }

    let title = if packages.len() == 1 {
        "Orphan packages (1)".to_string()
    } else {
        format!("Orphan packages ({})", packages.len())
    };

    let section = build_section_box(&title);

    let caption = Label::new(Some(
        "These packages are no longer required by anything else. You can safely remove them.",
    ));
    caption.add_css_class("dim-label");
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    section.card.append(&caption);

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");

    let checkboxes: Rc<RefCell<Vec<(String, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));

    for pkg in &packages {
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);

        let row_box = GtkBox::new(Orientation::Horizontal, 12);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);

        let check = CheckButton::new();
        check.set_active(true);
        check.set_valign(Align::Center);
        row_box.append(&check);

        let name_label = Label::new(Some(pkg));
        name_label.set_xalign(0.0);
        name_label.set_hexpand(true);
        row_box.append(&name_label);

        row.set_child(Some(&row_box));
        list_box.append(&row);

        checkboxes.borrow_mut().push((pkg.clone(), check));
    }

    section.card.append(&list_box);

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_top(4);

    let remove_btn = Button::with_label("Remove selected");
    remove_btn.add_css_class("destructive-action");
    button_row.append(&remove_btn);

    section.card.append(&button_row);

    let checkboxes_clone = checkboxes.clone();
    let window_clone = window.clone();
    remove_btn.connect_clicked(move |_| {
        let selected: Vec<String> = checkboxes_clone
            .borrow()
            .iter()
            .filter(|(_, c)| c.is_active())
            .map(|(name, _)| name.clone())
            .collect();

        log_info!(
            "post-update: Remove orphans clicked ({} selected)",
            selected.len()
        );
        if selected.is_empty() {
            return;
        }

        run_orphan_removal(&window_clone, selected);
    });

    page.sections_box.append(&section.wrapper);
    page.section_visibility.borrow_mut().orphans = true;
    refresh_all_clear(page);
}

pub fn set_services_section(page: &PostUpdatePage, services: Vec<String>) {
    if services.is_empty() {
        return;
    }

    let title = if services.len() == 1 {
        "Services that need restart (1)".to_string()
    } else {
        format!("Services that need restart ({})", services.len())
    };

    let section = build_section_box(&title);

    let caption = Label::new(Some(
        "These running services use files from packages that were just updated. A restart is needed to pick up the new versions.",
    ));
    caption.add_css_class("dim-label");
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    section.card.append(&caption);

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");

    let rows: Rc<RefCell<Vec<ServiceRowState>>> = Rc::new(RefCell::new(Vec::new()));

    for service in &services {
        let row_state = build_service_row(service);
        list_box.append(&row_state.row);
        rows.borrow_mut().push(row_state);
    }

    section.card.append(&list_box);

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_top(4);

    let restart_btn = Button::with_label("Restart selected");
    restart_btn.add_css_class("suggested-action");
    button_row.append(&restart_btn);

    section.card.append(&button_row);

    let rows_clone = rows.clone();
    let restart_btn_clone = restart_btn.clone();
    restart_btn.connect_clicked(move |_| {
        log_info!("post-update: Restart services clicked");
        restart_btn_clone.set_sensitive(false);
        let to_restart: Vec<usize> = rows_clone
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.check.is_active())
            .map(|(i, _)| i)
            .collect();

        for index in to_restart {
            kick_off_service_restart(rows_clone.clone(), index);
        }
    });

    page.sections_box.append(&section.wrapper);
    page.section_visibility.borrow_mut().services = true;
    refresh_all_clear(page);
}

pub fn run_post_update_command(window: &ApplicationWindow, command: &str) {
    let window_for_refresh = window.clone();
    run_command_in_dialog(window.upcast_ref::<gtk4::Window>(), command, move || {
        refresh_post_update(&window_for_refresh);
    });
}

pub fn refresh_post_update(window: &ApplicationWindow) {
    POST_UPDATE_PAGE.with(|cell| {
        if let Some(page) = cell.borrow().as_ref() {
            reset_post_update_page(page);
        }
    });
    run_post_update_detections(window.clone());
}

pub fn set_pacnew_section(page: &PostUpdatePage, files: Vec<String>, window: &ApplicationWindow) {
    if files.is_empty() {
        return;
    }

    let title = if files.len() == 1 {
        "Configuration files that need review (1)".to_string()
    } else {
        format!("Configuration files that need review ({})", files.len())
    };

    let section = build_section_box(&title);

    let caption = Label::new(Some(
        "Pacman saved updated versions of these system configuration files instead of overwriting your changes. Review and merge them so the new settings take effect.",
    ));
    caption.add_css_class("dim-label");
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    section.card.append(&caption);

    let meld_available = is_meld_available();

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");

    for file_path in &files {
        let row = build_pacnew_row(file_path, meld_available, window);
        list_box.append(&row);
    }

    section.card.append(&list_box);

    let footer_row = GtkBox::new(Orientation::Horizontal, 0);
    footer_row.set_halign(Align::End);
    footer_row.set_margin_top(4);

    let terminal_btn = Button::with_label("Open pacdiff in terminal");
    terminal_btn.add_css_class("flat");
    let window_clone = window.clone();
    terminal_btn.connect_clicked(move |_| {
        log_info!("post-update: Open pacdiff in terminal clicked");
        run_post_update_command(
            &window_clone,
            "DIFFPROG=${DIFFPROG:-vimdiff} sudo -E pacdiff",
        );
    });
    footer_row.append(&terminal_btn);

    section.card.append(&footer_row);

    page.sections_box.append(&section.wrapper);
    page.section_visibility.borrow_mut().pacnew = true;
    refresh_all_clear(page);
}

pub fn set_flatpak_unused_section(
    page: &PostUpdatePage,
    refs: Vec<(String, FlatpakInstallation)>,
    window: &ApplicationWindow,
) {
    if refs.is_empty() {
        return;
    }

    let title = if refs.len() == 1 {
        "Unused Flatpak runtimes (1)".to_string()
    } else {
        format!("Unused Flatpak runtimes ({})", refs.len())
    };

    let section = build_section_box(&title);

    let caption = Label::new(Some(
        "These Flatpak runtimes are no longer used by any installed app. Removing them frees up disk space.",
    ));
    caption.add_css_class("dim-label");
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    section.card.append(&caption);

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");

    let checkboxes: Rc<RefCell<Vec<(String, FlatpakInstallation, CheckButton)>>> =
        Rc::new(RefCell::new(Vec::new()));

    for (ref_name, installation) in &refs {
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);

        let row_box = GtkBox::new(Orientation::Horizontal, 12);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);

        let check = CheckButton::new();
        check.set_active(true);
        check.set_valign(Align::Center);
        row_box.append(&check);

        let scope_label = match installation {
            FlatpakInstallation::User => "user",
            FlatpakInstallation::System => "system",
        };
        let name_label = Label::new(Some(&format!("{} ({})", ref_name, scope_label)));
        name_label.set_xalign(0.0);
        name_label.set_hexpand(true);
        row_box.append(&name_label);

        row.set_child(Some(&row_box));
        list_box.append(&row);

        checkboxes
            .borrow_mut()
            .push((ref_name.clone(), *installation, check));
    }

    section.card.append(&list_box);

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_top(4);

    let remove_btn = Button::with_label("Remove selected");
    remove_btn.add_css_class("destructive-action");
    button_row.append(&remove_btn);

    section.card.append(&button_row);

    let checkboxes_clone = checkboxes.clone();
    let window_clone = window.clone();
    remove_btn.connect_clicked(move |_| {
        let selected: Vec<(String, FlatpakInstallation)> = checkboxes_clone
            .borrow()
            .iter()
            .filter(|(_, _, c)| c.is_active())
            .map(|(name, inst, _)| (name.clone(), *inst))
            .collect();

        log_info!(
            "post-update: Remove unused Flatpak clicked ({} selected)",
            selected.len()
        );
        if selected.is_empty() {
            return;
        }

        if let Some(command) = build_flatpak_uninstall_command(&selected) {
            run_post_update_command(&window_clone, &command);
        }
    });

    page.sections_box.append(&section.wrapper);
    page.section_visibility.borrow_mut().flatpak_unused = true;
    refresh_all_clear(page);
}

pub fn set_resolutions_section(
    page: &PostUpdatePage,
    switches: Vec<RepoSwitch>,
    window: &ApplicationWindow,
) {
    if switches.is_empty() {
        return;
    }

    let title = if switches.len() == 1 {
        "Package resolutions (1)".to_string()
    } else {
        format!("Package resolutions ({})", switches.len())
    };

    let section = build_section_box(&title);

    let caption = Label::new(Some(
        "These packages can be replaced by another one, or moved between repositories. Tick the ones you want to apply.",
    ));
    caption.add_css_class("dim-label");
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    section.card.append(&caption);

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");

    let checkboxes: Rc<RefCell<Vec<(RepoSwitch, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));

    for switch in &switches {
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);

        let row_box = GtkBox::new(Orientation::Horizontal, 12);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);

        let check = CheckButton::new();
        check.set_active(false);
        check.set_valign(Align::Center);
        row_box.append(&check);

        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        let title_label = Label::new(None);
        title_label.set_xalign(0.0);
        title_label.set_use_markup(true);
        title_label.set_markup(&format_resolution_title(switch));
        text_box.append(&title_label);

        let subtitle_label = Label::new(Some(&format_resolution_subtitle(switch)));
        subtitle_label.set_xalign(0.0);
        subtitle_label.add_css_class("dim-label");
        subtitle_label.add_css_class("caption");
        text_box.append(&subtitle_label);

        row_box.append(&text_box);
        row.set_child(Some(&row_box));
        list_box.append(&row);

        checkboxes.borrow_mut().push((switch.clone(), check));
    }

    section.card.append(&list_box);

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_top(4);

    let apply_btn = Button::with_label("Apply selected");
    apply_btn.add_css_class("suggested-action");
    button_row.append(&apply_btn);

    section.card.append(&button_row);

    let checkboxes_clone = checkboxes.clone();
    let window_clone = window.clone();
    apply_btn.connect_clicked(move |_| {
        let selected: Vec<String> = checkboxes_clone
            .borrow()
            .iter()
            .filter(|(_, c)| c.is_active())
            .map(|(s, _)| s.target_name.clone())
            .collect();

        log_info!(
            "post-update: Apply resolutions clicked ({} selected)",
            selected.len()
        );
        if selected.is_empty() {
            return;
        }

        let quoted: Vec<String> = selected
            .iter()
            .filter_map(|p| try_quote(p).ok().map(|c| c.into_owned()))
            .collect();

        if quoted.is_empty() {
            return;
        }

        let command = format!("sudo pacman -S {}", quoted.join(" "));
        run_post_update_command(&window_clone, &command);
    });

    page.sections_box.append(&section.wrapper);
    page.section_visibility.borrow_mut().resolutions = true;
    refresh_all_clear(page);
}

pub fn set_cache_section(
    page: &PostUpdatePage,
    candidates: CacheCandidates,
    keep_old: u32,
    keep_uninstalled: u32,
    window: &ApplicationWindow,
) {
    let total = candidates.old_count + candidates.uninstalled_count;
    if total == 0 {
        return;
    }

    let title = format!(
        "Cached packages ({} old, {} uninstalled)",
        candidates.old_count, candidates.uninstalled_count
    );
    let section = build_section_box(&title);

    let caption_text = match candidates.disk_space.as_deref() {
        Some(space) => format!("Cleaning will free about {} of disk space.", space),
        None => "Clean the pacman cache to free up disk space.".to_string(),
    };
    let caption = Label::new(Some(&caption_text));
    caption.add_css_class("dim-label");
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    section.card.append(&caption);

    let total_items = candidates.old_packages.len() + candidates.uninstalled_packages.len();
    if total_items > 0 {
        let model = StringList::new(&[]);
        for name in &candidates.old_packages {
            model.append(name);
        }
        for name in &candidates.uninstalled_packages {
            model.append(name);
        }

        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let Some(item) = list_item.downcast_ref::<ListItem>() else {
                return;
            };
            let label = Label::new(None);
            label.set_xalign(0.0);
            label.set_margin_start(12);
            label.set_margin_end(12);
            label.set_margin_top(6);
            label.set_margin_bottom(6);
            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            item.set_child(Some(&label));
        });
        factory.connect_bind(|_, list_item| {
            let Some(item) = list_item.downcast_ref::<ListItem>() else {
                return;
            };
            let Some(name_obj) = item.item().and_downcast::<StringObject>() else {
                return;
            };
            let Some(label) = item.child().and_downcast::<Label>() else {
                return;
            };
            label.set_text(&name_obj.string());
        });

        let selection = NoSelection::new(Some(model));
        let list_view = ListView::new(Some(selection), Some(factory));
        list_view.set_single_click_activate(false);

        let list_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .max_content_height(240)
            .propagate_natural_height(true)
            .child(&list_view)
            .build();

        section.card.append(&list_scroll);
    }

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_top(4);

    let clean_btn = Button::with_label("Clean cache");
    clean_btn.add_css_class("destructive-action");
    button_row.append(&clean_btn);

    section.card.append(&button_row);

    let window_clone = window.clone();
    clean_btn.connect_clicked(move |_| {
        log_info!("post-update: Clean cache clicked");
        let command = format!(
            "sudo paccache -rk{} && sudo paccache -ruk{}",
            keep_old, keep_uninstalled
        );
        run_post_update_command(&window_clone, &command);
    });

    page.sections_box.append(&section.wrapper);
    page.section_visibility.borrow_mut().cache = true;
    refresh_all_clear(page);
}

pub fn run_post_update_detections(window: ApplicationWindow) {
    let settings = load_settings();
    let keep_old = settings.keep_old_packages;
    let keep_uninstalled = settings.keep_uninstalled_packages;
    let flatpak_enabled = settings.enable_flatpak_support;
    let auto_clean_cache = settings.auto_clean_cache;

    glib::spawn_future_local(async move {
        let orphans_result = gio::spawn_blocking(|| get_orphan_packages()).await;
        let orphans = match orphans_result {
            Ok(Ok(list)) => list,
            Ok(Err(e)) => {
                eprintln!("Failed to detect orphan packages: {}", e);
                Vec::new()
            }
            Err(e) => {
                eprintln!("Orphan detection thread failed: {:?}", e);
                Vec::new()
            }
        };

        if auto_clean_cache {
            match gio::spawn_blocking(move || clean_cache(keep_old, keep_uninstalled)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("Failed to auto-clean cache: {}", e),
                Err(e) => eprintln!("Cache clean thread failed: {:?}", e),
            }
        }

        let cache_result =
            gio::spawn_blocking(move || get_cache_candidates(keep_old, keep_uninstalled)).await;
        let cache = match cache_result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                eprintln!("Failed to detect cache candidates: {}", e);
                Default::default()
            }
            Err(e) => {
                eprintln!("Cache detection thread failed: {:?}", e);
                Default::default()
            }
        };

        let reboot_pending = gio::spawn_blocking(|| is_kernel_reboot_pending())
            .await
            .unwrap_or(false);

        let pacnew_result = gio::spawn_blocking(|| get_pacnew_files()).await;
        let pacnew = match pacnew_result {
            Ok(Ok(list)) => list,
            Ok(Err(e)) => {
                eprintln!("Failed to detect pacnew files: {}", e);
                Vec::new()
            }
            Err(e) => {
                eprintln!("Pacnew detection thread failed: {:?}", e);
                Vec::new()
            }
        };

        let services_result = gio::spawn_blocking(|| get_services_needing_restart()).await;
        let services = match services_result {
            Ok(Ok(list)) => list,
            Ok(Err(e)) => {
                eprintln!("Failed to detect services needing restart: {}", e);
                Vec::new()
            }
            Err(e) => {
                eprintln!("Services detection thread failed: {:?}", e);
                Vec::new()
            }
        };

        let switches_result = gio::spawn_blocking(|| detect_repo_switches()).await;
        let switches = match switches_result {
            Ok(Ok(list)) => list,
            Ok(Err(e)) => {
                eprintln!("Failed to detect package resolutions: {}", e);
                Vec::new()
            }
            Err(e) => {
                eprintln!("Resolutions detection thread failed: {:?}", e);
                Vec::new()
            }
        };

        let flatpak_unused = if flatpak_enabled {
            let result = gio::spawn_blocking(|| get_unused_flatpak_runtimes()).await;
            match result {
                Ok(Ok(list)) => list,
                Ok(Err(e)) => {
                    eprintln!("Failed to detect unused Flatpak runtimes: {}", e);
                    Vec::new()
                }
                Err(e) => {
                    eprintln!("Flatpak unused detection thread failed: {:?}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        POST_UPDATE_PAGE.with(|cell| {
            if let Some(page) = cell.borrow().as_ref() {
                set_orphans_section(page, orphans, &window);
                set_cache_section(page, cache, keep_old, keep_uninstalled, &window);
                set_pacnew_section(page, pacnew, &window);
                set_services_section(page, services);
                set_flatpak_unused_section(page, flatpak_unused, &window);
                set_resolutions_section(page, switches, &window);
                page.reboot_banner.set_visible(reboot_pending);
                finish_post_update_loading(page);
            }
        });
    });
}

fn build_loading_box() -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 12);
    outer.set_halign(Align::Center);
    outer.set_valign(Align::Center);
    outer.set_margin_top(48);
    outer.set_margin_bottom(48);

    let spinner = Spinner::new();
    spinner.set_size_request(40, 40);
    spinner.set_widget_name("post-update-spinner");
    outer.append(&spinner);

    let label = Label::new(Some("Running post-update checks..."));
    label.add_css_class("dim-label");
    outer.append(&label);

    outer.set_visible(false);

    return outer;
}

fn start_loading_spinner(loading_box: &GtkBox) {
    let mut child = loading_box.first_child();
    while let Some(widget) = child {
        if let Some(spinner) = widget.downcast_ref::<Spinner>() {
            spinner.set_spinning(true);
            return;
        }
        child = widget.next_sibling();
    }
}

fn stop_loading_spinner(loading_box: &GtkBox) {
    let mut child = loading_box.first_child();
    while let Some(widget) = child {
        if let Some(spinner) = widget.downcast_ref::<Spinner>() {
            spinner.set_spinning(false);
            return;
        }
        child = widget.next_sibling();
    }
}

fn build_reboot_banner() -> GtkBox {
    let banner = GtkBox::new(Orientation::Horizontal, 12);
    banner.add_css_class("reboot-banner");
    banner.set_margin_start(12);
    banner.set_margin_end(12);
    banner.set_margin_top(12);

    let icon = Image::from_icon_name("system-reboot-symbolic");
    icon.set_pixel_size(24);
    banner.append(&icon);

    let text_box = GtkBox::new(Orientation::Vertical, 2);
    text_box.set_hexpand(true);

    let title = Label::new(Some("A reboot is required"));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    text_box.append(&title);

    let body = Label::new(Some(
        "A new kernel was installed. Restart your system to finish applying the update.",
    ));
    body.add_css_class("dim-label");
    body.set_xalign(0.0);
    body.set_wrap(true);
    text_box.append(&body);

    banner.append(&text_box);

    let reboot_button = Button::with_label("Reboot now");
    reboot_button.add_css_class("destructive-action");
    reboot_button.set_valign(Align::Center);
    reboot_button.connect_clicked(|button| {
        log_info!("post-update: Reboot now clicked");
        let parent = button.root().and_downcast::<gtk4::Window>();
        prompt_reboot(parent.as_ref());
    });
    banner.append(&reboot_button);

    banner.set_visible(false);

    return banner;
}

fn prompt_reboot(parent: Option<&gtk4::Window>) {
    let alert = gtk4::AlertDialog::builder()
        .modal(true)
        .message("Reboot now?")
        .detail("Your system will restart immediately. Save your work first.")
        .buttons(["Cancel", "Reboot"])
        .cancel_button(0)
        .default_button(1)
        .build();

    alert.choose(parent, gio::Cancellable::NONE, |result| {
        if let Ok(1) = result {
            let _ = std::process::Command::new("systemctl")
                .arg("reboot")
                .spawn();
        }
    });
}

fn build_all_clear_box() -> GtkBox {
    let outer = GtkBox::new(Orientation::Horizontal, 12);
    outer.set_halign(Align::Center);
    outer.set_margin_top(24);
    outer.set_margin_bottom(24);

    let icon = Image::from_icon_name("object-select-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("success");
    outer.append(&icon);

    let text_box = GtkBox::new(Orientation::Vertical, 4);
    text_box.set_valign(Align::Center);

    let title = Label::new(Some("All post-update checks passed"));
    title.add_css_class("title-3");
    title.set_xalign(0.0);
    text_box.append(&title);

    let subtitle = Label::new(Some("Your system is fully up to date."));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    text_box.append(&subtitle);

    outer.append(&text_box);

    return outer;
}

fn install_post_update_css() {
    use std::sync::OnceLock;
    static CSS_INSTALLED: OnceLock<()> = OnceLock::new();

    CSS_INSTALLED.get_or_init(|| {
        let Some(display) = gtk4::gdk::Display::default() else {
            return;
        };

        let provider = gtk4::CssProvider::new();
        provider.load_from_data(
            ".post-update-card {
                background-color: alpha(currentColor, 0.05);
                border: 1px solid alpha(currentColor, 0.1);
                border-radius: 12px;
                padding: 14px;
            }
            .post-update-card list.boxed-list {
                background-color: transparent;
                border: none;
            }
            .post-update-card list.boxed-list > row {
                background-color: transparent;
            }",
        );

        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}

fn run_orphan_removal(window: &ApplicationWindow, packages: Vec<String>) {
    let quoted: Vec<String> = packages
        .iter()
        .filter_map(|p| try_quote(p).ok().map(|c| c.into_owned()))
        .collect();

    if quoted.is_empty() {
        return;
    }

    let command = format!("sudo pacman -Rns {}", quoted.join(" "));
    run_post_update_command(window, &command);
}

fn build_pacnew_row(
    file_path: &str,
    meld_available: bool,
    window: &ApplicationWindow,
) -> ListBoxRow {
    let _ = window;

    let row = ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let row_box = GtkBox::new(Orientation::Horizontal, 12);
    row_box.set_margin_start(12);
    row_box.set_margin_end(12);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);

    let path_label = Label::new(Some(file_path));
    path_label.set_xalign(0.0);
    path_label.set_hexpand(true);
    path_label.set_wrap(true);
    path_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    row_box.append(&path_label);

    let diff_btn = Button::with_label("Diff");
    diff_btn.add_css_class("suggested-action");
    let path_for_diff = file_path.to_string();
    diff_btn.connect_clicked(move |btn| {
        log_info!("post-update: Diff clicked for {}", path_for_diff);
        let parent = btn.root().and_downcast::<gtk4::Window>();
        if let Some(parent_window) = parent {
            show_pacnew_diff_dialog(&parent_window, &path_for_diff);
        }
    });
    row_box.append(&diff_btn);

    if meld_available {
        let meld_btn = Button::with_label("Open in meld");
        meld_btn.add_css_class("flat");
        let path_owned = file_path.to_string();
        meld_btn.connect_clicked(move |_| {
            log_info!("post-update: Open in meld clicked for {}", path_owned);
            open_meld(&path_owned);
        });
        row_box.append(&meld_btn);
    }

    row.set_child(Some(&row_box));
    return row;
}

fn open_meld(saved_path: &str) {
    let original = strip_pacnew_suffix(saved_path);
    spawn_as_user_or_root("meld", &[&original, saved_path]);
}

fn strip_pacnew_suffix(path: &str) -> String {
    for suffix in [".pacnew", ".pacsave", ".pacorig"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    return path.to_string();
}

fn format_resolution_title(switch: &RepoSwitch) -> String {
    let escape = |s: &str| glib::markup_escape_text(s).to_string();
    return match switch.kind {
        SwitchKind::RepoChange => format!(
            "<b>{}</b>: {} to {}",
            escape(&switch.installed_name),
            escape(&switch.installed_repo),
            escape(&switch.target_repo),
        ),
        SwitchKind::Replace => format!(
            "<b>{}</b> to <b>{}/{}</b>",
            escape(&switch.installed_name),
            escape(&switch.target_repo),
            escape(&switch.target_name),
        ),
    };
}

fn format_resolution_subtitle(switch: &RepoSwitch) -> String {
    return match switch.kind {
        SwitchKind::RepoChange => {
            if switch.installed_version == switch.target_version {
                format!("version {} (same)", switch.installed_version)
            } else {
                format!(
                    "version {} to {}",
                    switch.installed_version, switch.target_version
                )
            }
        }
        SwitchKind::Replace => format!(
            "replaces {} ({} to {})",
            switch.installed_name, switch.installed_version, switch.target_version
        ),
    };
}

fn build_service_row(service: &str) -> ServiceRowState {
    let row = ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let row_box = GtkBox::new(Orientation::Horizontal, 12);
    row_box.set_margin_start(12);
    row_box.set_margin_end(12);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);

    let check = CheckButton::new();
    check.set_active(true);
    check.set_valign(Align::Center);
    row_box.append(&check);

    let name_label = Label::new(Some(service));
    name_label.set_xalign(0.0);
    name_label.set_hexpand(true);
    row_box.append(&name_label);

    let status_box = GtkBox::new(Orientation::Horizontal, 6);
    status_box.set_valign(Align::Center);
    row_box.append(&status_box);

    row.set_child(Some(&row_box));

    return ServiceRowState {
        row,
        name: service.to_string(),
        check,
        status_box,
    };
}

fn kick_off_service_restart(rows: Rc<RefCell<Vec<ServiceRowState>>>, index: usize) {
    let service_name;
    {
        let rows_borrow = rows.borrow();
        let Some(row) = rows_borrow.get(index) else {
            return;
        };
        row.check.set_sensitive(false);
        clear_box(&row.status_box);
        let spinner = Spinner::new();
        spinner.set_spinning(true);
        spinner.set_size_request(16, 16);
        row.status_box.append(&spinner);
        service_name = row.name.clone();
    }

    glib::spawn_future_local(async move {
        let outcome = gio::spawn_blocking(move || restart_service(&service_name)).await;

        let outcome = match outcome {
            Ok(o) => o,
            Err(e) => ServiceRestartOutcome {
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("Restart task failed: {:?}", e),
            },
        };

        let rows_borrow = rows.borrow();
        let Some(row) = rows_borrow.get(index) else {
            return;
        };

        clear_box(&row.status_box);

        if outcome.success {
            let icon = Image::from_icon_name("object-select-symbolic");
            icon.add_css_class("success");
            row.status_box.append(&icon);

            let done = Label::new(Some("Done"));
            done.add_css_class("dim-label");
            row.status_box.append(&done);
        } else {
            let icon = Image::from_icon_name("dialog-error-symbolic");
            icon.add_css_class("error");
            row.status_box.append(&icon);

            let error_btn = Button::with_label("Error");
            error_btn.add_css_class("flat");
            let error_text = format_service_error(&outcome);
            let row_name = row.name.clone();
            error_btn.connect_clicked(move |btn| {
                let parent = btn.root().and_downcast::<gtk4::Window>();
                show_service_error_dialog(parent.as_ref(), &row_name, &error_text);
            });
            row.status_box.append(&error_btn);
        }
    });
}

fn clear_box(container: &GtkBox) {
    let mut child = container.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        container.remove(&widget);
        child = next;
    }
}

fn format_service_error(outcome: &ServiceRestartOutcome) -> String {
    let mut out = String::new();

    if let Some(code) = outcome.exit_code {
        out.push_str(&format!("Exit code: {}\n", code));
    } else {
        out.push_str("Exit code: unknown\n");
    }

    if !outcome.stdout.trim().is_empty() {
        out.push_str("\nStandard output:\n");
        out.push_str(&outcome.stdout);
    }

    if !outcome.stderr.trim().is_empty() {
        out.push_str("\nStandard error:\n");
        out.push_str(&outcome.stderr);
    }

    return out;
}

fn show_service_error_dialog(parent: Option<&gtk4::Window>, service_name: &str, content: &str) {
    let window = gtk4::Window::builder()
        .title(&format!("Error restarting {}", service_name))
        .modal(true)
        .default_width(540)
        .default_height(360)
        .build();

    if let Some(parent_window) = parent {
        window.set_transient_for(Some(parent_window));
    }

    let root = GtkBox::new(Orientation::Vertical, 0);

    let scrolled = ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    let text = gtk4::TextView::new();
    text.set_editable(false);
    text.set_cursor_visible(false);
    text.set_monospace(true);
    text.set_top_margin(12);
    text.set_bottom_margin(12);
    text.set_left_margin(12);
    text.set_right_margin(12);
    text.buffer().set_text(content);

    scrolled.set_child(Some(&text));
    root.append(&scrolled);

    root.append(&Separator::new(Orientation::Horizontal));

    let button_row = GtkBox::new(Orientation::Horizontal, 0);
    button_row.set_halign(Align::End);
    button_row.set_margin_start(8);
    button_row.set_margin_end(8);
    button_row.set_margin_top(8);
    button_row.set_margin_bottom(8);

    let close_button = Button::with_label("Close");
    let window_for_close = window.clone();
    close_button.connect_clicked(move |_| window_for_close.close());
    button_row.append(&close_button);
    root.append(&button_row);

    window.set_child(Some(&root));
    window.present();
}
