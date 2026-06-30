use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, Orientation, Popover, PositionType, Widget,
};

use crate::helpers::elevated::open_url_as_user;
use crate::helpers::pacman_ignore::{
    add_to_ignore_pkg, is_in_managed_ignore_pkg, remove_from_ignore_pkg,
};
use crate::helpers::settings::{load_settings, save_settings};
use crate::helpers::tray_integration::{kick_tray, trigger_check_service};
use crate::log_info;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;
use crate::ui::aur_scan_dialog::show_aur_scan_dialog;
use crate::ui::dialogs::show_error_dialog;
use crate::ui::downgrade_dialog::show_downgrade_dialog;
use crate::ui::main_window::load_packages;
use crate::ui::package_files_dialog::show_package_files_dialog;
use crate::ui::package_list::refresh_favorite_button;
use crate::ui::pkgbuild_review_dialog::show_pkgbuild_review_dialog;

pub fn show_package_context_menu(anchor: &Widget, package: &PackageUpdate, x: f64, y: f64) {
    let Some(window) = anchor.root().and_downcast::<ApplicationWindow>() else {
        return;
    };

    log_info!("context menu opened for {}", package.name);

    let popover = Popover::new();
    popover.set_position(PositionType::Bottom);
    popover.set_has_arrow(false);
    popover.set_autohide(true);

    let rect = Rectangle::new(x as i32, y as i32, 1, 1);
    popover.set_pointing_to(Some(&rect));

    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.set_margin_top(4);
    vbox.set_margin_bottom(4);
    vbox.set_margin_start(4);
    vbox.set_margin_end(4);

    let is_aur = package.source == PackageSource::Aur;
    let is_external =
        package.source == PackageSource::Flatpak || package.source == PackageSource::AppImage;

    add_action(&vbox, &popover, "Copy package name", {
        let name = package.name.clone();
        let widget = anchor.clone();
        move || {
            widget.display().clipboard().set_text(&name);
        }
    });

    if is_aur {
        add_action(&vbox, &popover, "Open AUR page", {
            let name = package.name.clone();
            move || {
                open_url_as_user(&format!("https://aur.archlinux.org/packages/{}", name));
            }
        });

        add_action(&vbox, &popover, "Review PKGBUILD changes", {
            let name = package.name.clone();
            let window = window.clone();
            move || {
                show_pkgbuild_review_dialog(window.upcast_ref::<gtk4::Window>(), &name);
            }
        });

        if !package.aur_scan_findings.is_empty() {
            add_action(&vbox, &popover, "View aur-scan results", {
                let name = package.name.clone();
                let window = window.clone();
                move || {
                    show_aur_scan_dialog(window.upcast_ref::<gtk4::Window>(), &name);
                }
            });
        }
    } else if !is_external {
        add_action(&vbox, &popover, "Open Arch package page", {
            let name = package.name.clone();
            move || {
                open_url_as_user(&format!("https://archlinux.org/packages/?q={}", name));
            }
        });
    }

    if !is_external {
        add_action(&vbox, &popover, "View files", {
            let name = package.name.clone();
            let window = window.clone();
            move || {
                show_package_files_dialog(&window, &name);
            }
        });
    }

    add_separator(&vbox);

    let settings = load_settings();
    let is_favorite = settings.is_favorite(&package.name);
    add_action(
        &vbox,
        &popover,
        if is_favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        },
        {
            let name = package.name.clone();
            let window = window.clone();
            move || {
                toggle_favorite(&window, &name);
            }
        },
    );

    if !is_external {
        let is_blacklisted = is_in_managed_ignore_pkg(&package.name);
        add_action(
            &vbox,
            &popover,
            if is_blacklisted {
                "Remove from blacklist"
            } else {
                "Add to blacklist"
            },
            {
                let name = package.name.clone();
                let window = window.clone();
                move || {
                    toggle_blacklist(&window, &name, !is_blacklisted);
                }
            },
        );

        add_separator(&vbox);

        add_action(&vbox, &popover, "Downgrade...", {
            let name = package.name.clone();
            let current_version = package.current_version.clone();
            let window = window.clone();
            move || {
                show_downgrade_dialog(&window, &name, &current_version);
            }
        });
    }

    popover.set_child(Some(&vbox));
    popover.set_parent(anchor);

    let popover_for_cleanup = popover.clone();
    popover.connect_closed(move |_| {
        popover_for_cleanup.unparent();
    });

    popover.popup();
}

fn add_action<F>(parent: &GtkBox, popover: &Popover, label: &str, action: F)
where
    F: Fn() + 'static,
{
    let button = Button::with_label(label);
    button.add_css_class("flat");
    button.set_halign(Align::Fill);
    let label_widget = button.child().and_downcast::<gtk4::Label>();
    if let Some(label_widget) = label_widget {
        label_widget.set_xalign(0.0);
        label_widget.set_halign(Align::Start);
    }
    let popover_clone = popover.clone();
    let label_owned = label.to_string();
    button.connect_clicked(move |_| {
        log_info!("context menu action: {}", label_owned);
        popover_clone.popdown();
        action();
    });
    parent.append(&button);
}

fn add_separator(parent: &GtkBox) {
    let sep = gtk4::Separator::new(Orientation::Horizontal);
    sep.set_margin_top(4);
    sep.set_margin_bottom(4);
    parent.append(&sep);
}

fn toggle_favorite(window: &ApplicationWindow, name: &str) {
    let mut settings = load_settings();
    let is_now_favorite = !settings.is_favorite(name);
    settings.set_favorite(name, is_now_favorite);
    if let Err(e) = save_settings(&settings) {
        show_error_dialog(
            window.upcast_ref::<gtk4::Window>(),
            "Failed to save favorites",
            &format!("{}", e),
        );
        return;
    }
    refresh_favorite_button(name, is_now_favorite);
    kick_tray();
}

fn toggle_blacklist(window: &ApplicationWindow, name: &str, add: bool) {
    let result = if add {
        add_to_ignore_pkg(name)
    } else {
        remove_from_ignore_pkg(name)
    };
    if let Err(e) = result {
        show_error_dialog(
            window.upcast_ref::<gtk4::Window>(),
            "Failed to update pacman.conf",
            &format!("{}", e),
        );
        return;
    }
    trigger_check_service();
    reload_package_list(window);
}

pub fn reload_package_list(window: &ApplicationWindow) {
    let Some(main_box) = window.child().and_downcast::<GtkBox>() else {
        return;
    };
    let Some(stack) = main_box.first_child().and_downcast::<gtk4::Stack>() else {
        return;
    };
    let Some(content_box) = stack.child_by_name("content").and_downcast::<GtkBox>() else {
        return;
    };
    stack.set_visible_child_name("loading");
    load_packages(stack, content_box, window.clone());
}
