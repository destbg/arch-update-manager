use crate::constants::TIMESHIFT_COMMENT;
use crate::helpers::get_navigation_stack::get_navigation_stack;
use crate::helpers::timeshift::{create_timeshift_snapshot, delete_old_timeshift_snapshot};
use crate::models::package_object::PackageUpdateObject;
use crate::ui::dialogs::{create_progress_dialog, show_error_dialog};
use crate::ui::package_list::update_statusbar;
use gio::ListStore;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, Box as GtkBox, Button, CheckButton, ColumnView, Frame, Image, Orientation,
    Paned, ScrolledWindow, Separator, SingleSelection, Stack, Statusbar,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use vte4::prelude::*;

pub fn create_toolbar() -> (GtkBox, CheckButton) {
    let toolbar_container = GtkBox::new(Orientation::Vertical, 6);
    toolbar_container.set_margin_start(6);
    toolbar_container.set_margin_end(6);
    toolbar_container.set_margin_top(6);
    toolbar_container.set_margin_bottom(6);

    let toolbar = GtkBox::new(Orientation::Horizontal, 6);

    let clear_btn = Button::new();
    clear_btn.add_css_class("destructive-action");
    clear_btn.set_child(Some(&create_button_content("edit-clear", "Clear")));
    clear_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            if let Some((store, statusbar)) = find_store_and_statusbar(&toolbar) {
                clear_all_selections(&store, &statusbar);
            }
        }
    ));
    toolbar.append(&clear_btn);

    let select_all_btn = Button::new();
    select_all_btn.set_child(Some(&create_button_content(
        "edit-select-all",
        "Select All",
    )));
    select_all_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            if let Some((store, statusbar)) = find_store_and_statusbar(&toolbar) {
                select_all_packages(&store, &statusbar);
            }
        }
    ));
    toolbar.append(&select_all_btn);

    let separator = Separator::new(Orientation::Vertical);
    toolbar.append(&separator);

    let refresh_btn = Button::new();
    refresh_btn.set_child(Some(&create_button_content("view-refresh", "Refresh")));

    refresh_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        move |_| {
            let Some((stack, content_box, window)) = get_navigation_stack(&toolbar) else {
                return;
            };

            stack.set_visible_child_name("loading");
            crate::ui::main_window::load_packages(stack, content_box, window);
        }
    ));

    toolbar.append(&refresh_btn);

    let separator2 = Separator::new(Orientation::Vertical);
    toolbar.append(&separator2);

    let timeshift_checkbox = CheckButton::with_label("Create Timeshift snapshot before update");

    let install_btn = Button::new();
    install_btn.add_css_class("suggested-action");
    install_btn.set_child(Some(&create_button_content(
        "system-software-install",
        "Install Updates",
    )));
    install_btn.connect_clicked(clone!(
        #[weak]
        toolbar,
        #[weak]
        timeshift_checkbox,
        move |_| {
            if let Some((store, _statusbar)) = find_store_and_statusbar(&toolbar) {
                if let Some(window) = toolbar.root().and_downcast::<ApplicationWindow>() {
                    let create_snapshot = timeshift_checkbox.is_active();
                    if let Err(e) = install_selected_packages_ui(&store, &window, create_snapshot) {
                        eprintln!("Failed to install packages: {}", e);
                    }
                }
            }
        }
    ));
    toolbar.append(&install_btn);

    toolbar_container.append(&toolbar);
    timeshift_checkbox.set_active(true);
    timeshift_checkbox.set_margin_start(6);
    timeshift_checkbox.set_margin_end(6);
    timeshift_checkbox.set_margin_top(3);

    toolbar_container.append(&timeshift_checkbox);

    return (toolbar_container, timeshift_checkbox);
}

fn find_store_and_statusbar(toolbar: &GtkBox) -> Option<(ListStore, Statusbar)> {
    let Some((_, content_box, _)) = get_navigation_stack(toolbar) else {
        return None;
    };

    let Some(paned) = content_box
        .last_child()
        .and_then(|child| child.prev_sibling())
        .and_downcast::<Paned>()
    else {
        return None;
    };

    let Some(scrolled) = paned.start_child().and_downcast::<ScrolledWindow>() else {
        return None;
    };

    let Some(column_view) = scrolled.child().and_downcast::<ColumnView>() else {
        return None;
    };

    let Some(selection_model) = column_view.model() else {
        return None;
    };

    let Some(list_store) = selection_model
        .downcast_ref::<SingleSelection>()
        .and_then(|sm| sm.model())
        .and_downcast::<ListStore>()
    else {
        return None;
    };

    let Some(statusbar) = content_box.last_child().and_downcast::<Statusbar>() else {
        return None;
    };

    return Some((list_store, statusbar));
}

fn clear_all_selections(store: &ListStore, statusbar: &Statusbar) {
    let n_items = store.n_items();
    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            item.set_selected(false);
        }
    }
    let items: Vec<PackageUpdateObject> = (0..n_items)
        .filter_map(|i| store.item(i).and_downcast::<PackageUpdateObject>())
        .collect();

    store.remove_all();
    for item in items {
        store.append(&item);
    }

    update_statusbar(statusbar, store);
}

fn select_all_packages(store: &ListStore, statusbar: &Statusbar) {
    let n_items = store.n_items();
    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            item.set_selected(true);
        }
    }
    let items: Vec<PackageUpdateObject> = (0..n_items)
        .filter_map(|i| store.item(i).and_downcast::<PackageUpdateObject>())
        .collect();

    store.remove_all();
    for item in items {
        store.append(&item);
    }

    update_statusbar(statusbar, store);
}

fn install_selected_packages_ui(
    store: &ListStore,
    window: &ApplicationWindow,
    create_snapshot: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut selected_packages = Vec::new();
    let n_items = store.n_items();

    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            let data = item.data();
            if data.selected {
                selected_packages.push(data.name);
            }
        }
    }

    if selected_packages.is_empty() {
        return Ok(());
    }

    if create_snapshot {
        let progress_dialog = create_progress_dialog(
            &window.upcast_ref::<gtk4::Window>(),
            "Creating System Snapshot",
            "Creating Timeshift snapshot and deleting old snapshots...\n\nPlease wait, this may take a few minutes.",
        );
        progress_dialog.show();

        execute_timeshift_operations_async(
            selected_packages.clone(),
            window.clone(),
            progress_dialog,
        );

        return Ok(());
    }

    if let Err(e) = navigate_to_terminal_and_install(window, selected_packages) {
        show_error_dialog(
            &window.upcast_ref::<gtk4::Window>(),
            "Installation Error",
            &format!("Failed to start installation: {}", e),
        );
    }
    return Ok(());
}

fn execute_timeshift_operations_async(
    selected_packages: Vec<String>,
    window: ApplicationWindow,
    progress_dialog: gtk4::Dialog,
) {
    let (tx, rx) = mpsc::channel();
    let selected_packages_clone = selected_packages.clone();

    thread::spawn(move || match create_timeshift_snapshot(TIMESHIFT_COMMENT) {
        Ok(newest) => match delete_old_timeshift_snapshot(TIMESHIFT_COMMENT, &newest) {
            Ok(()) => {
                let _ = tx.send(("success", "Package installation starting".to_string()));
            }
            Err(e) => {
                let _ = tx.send(("error", format!("Failed to clean up old snapshots: {}", e)));
            }
        },
        Err(e) => {
            let _ = tx.send(("error", format!("Failed to create system snapshot: {}", e)));
        }
    });

    glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
        Ok(("success", _)) => {
            progress_dialog.close();

            if let Err(e) =
                navigate_to_terminal_and_install(&window, selected_packages_clone.clone())
            {
                show_error_dialog(
                    &window.upcast_ref::<gtk4::Window>(),
                    "Installation Error",
                    &format!("Failed to start installation: {}", e),
                );
            }

            glib::ControlFlow::Break
        }
        Ok(("error", message)) => {
            progress_dialog.close();
            show_error_dialog(
                &window.upcast_ref::<gtk4::Window>(),
                "Timeshift Error",
                &message,
            );
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => {
            progress_dialog.close();
            glib::ControlFlow::Break
        }
        _ => glib::ControlFlow::Continue,
    });
}

fn create_button_content(icon_name: &str, label_text: &str) -> GtkBox {
    let button_box = GtkBox::new(Orientation::Horizontal, 6);
    button_box.set_halign(gtk4::Align::Center);

    let icon = Image::from_icon_name(icon_name);
    let label = gtk4::Label::new(Some(label_text));

    button_box.append(&icon);
    button_box.append(&label);

    return button_box;
}

fn find_terminal_in_box(container: &GtkBox) -> Option<Frame> {
    let mut child = container.first_child();
    while let Some(widget) = child {
        if let Some(frame) = widget.downcast_ref::<Frame>() {
            if frame.label().is_some_and(|label| label == "Terminal") {
                return Some(frame.clone());
            }
        }

        if let Some(child_box) = widget.downcast_ref::<GtkBox>() {
            if let Some(found) = find_terminal_in_box(child_box) {
                return Some(found);
            }
        }
        child = widget.next_sibling();
    }
    return None;
}

fn start_installation_in_terminal(
    terminal: &vte4::Terminal,
    packages: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command_args = vec!["sudo".to_string(), "pacman".to_string(), "-S".to_string()];
    command_args.extend(packages);

    let args: Vec<&str> = command_args.iter().map(|s| s.as_str()).collect();

    terminal.spawn_async(
        vte4::PtyFlags::DEFAULT,   // no special flags
        None,                      // default working directory
        &args,                     // command arguments
        &[],                       // default environment
        glib::SpawnFlags::DEFAULT, // no special flags
        || {},                     // child setup function
        -1,                        // timeout
        None::<&gio::Cancellable>, // cancellable
        |result| {
            if let Err(e) = result {
                eprintln!("Failed to spawn pacman in terminal: {}", e);
            }
        },
    );

    return Ok(());
}

fn navigate_to_terminal_and_install(
    window: &ApplicationWindow,
    packages: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(main_box) = window.child().and_downcast::<GtkBox>() else {
        return Err("Could not find main box".into());
    };
    let Some(stack) = main_box.first_child().and_downcast::<Stack>() else {
        return Err("Could not find stack".into());
    };
    let Some(terminal_box) = stack.child_by_name("terminal").and_downcast::<GtkBox>() else {
        return Err("Could not find terminal box".into());
    };
    let Some(terminal_frame) = find_terminal_in_box(&terminal_box) else {
        return Err("Could not find terminal frame".into());
    };
    let Some(terminal) = terminal_frame.child().and_downcast::<vte4::Terminal>() else {
        return Err("Could not find terminal widget".into());
    };

    stack.set_visible_child_name("terminal");
    start_installation_in_terminal(&terminal, packages)?;

    return Ok(());
}
