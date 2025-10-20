use crate::helpers::package_updates::get_package_updates;
use crate::helpers::settings::load_settings;
use crate::models::package_object::PackageUpdateObject;
use crate::ui::dialogs::show_error_dialog;
use crate::ui::info_panel::create_info_panel;
use crate::ui::loading::create_loading_page;
use crate::ui::no_updates::create_no_updates_page;
use crate::ui::package_list::{create_package_list, update_statusbar};
use crate::ui::settings_dialog::show_settings_dialog;
use crate::ui::terminal_page::create_terminal_page;
use crate::ui::toolbar::create_toolbar;
use gio::ListStore;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ColumnView, HeaderBar, Orientation,
    Paned, ScrolledWindow, Separator, SingleSelection, Stack, Statusbar,
};

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Arch Update Manager")
        .icon_name("arch-update-manager")
        .default_width(900)
        .default_height(600)
        .build();

    let header_bar = HeaderBar::new();
    header_bar.set_title_widget(Some(&gtk4::Label::new(Some("Arch Update Manager"))));

    let settings_button = Button::from_icon_name("preferences-system-symbolic");
    settings_button.set_tooltip_text(Some("Settings"));

    let window_clone = window.clone();
    settings_button.connect_clicked(move |_| {
        let settings = load_settings();
        show_settings_dialog(&window_clone, &settings);
    });

    header_bar.pack_end(&settings_button);
    window.set_titlebar(Some(&header_bar));

    let main_box = GtkBox::new(Orientation::Vertical, 0);

    let stack = Stack::new();
    stack.set_vexpand(true);

    let loading_box = create_loading_page();
    stack.add_named(&loading_box, Some("loading"));

    let no_updates_box = create_no_updates_page();
    stack.add_named(&no_updates_box, Some("no-updates"));

    let terminal_box = create_terminal_page();
    stack.add_named(&terminal_box, Some("terminal"));

    let content_box = create_main_content();
    stack.add_named(&content_box, Some("content"));

    main_box.append(&stack);

    window.set_child(Some(&main_box));

    stack.set_visible_child_name("loading");

    window.present();

    let stack_clone = stack.clone();
    let content_box_clone = content_box.clone();
    let window_clone2 = window.clone();
    glib::idle_add_local_once(move || {
        load_packages(stack_clone, content_box_clone, window_clone2);
    });
}

fn create_main_content() -> GtkBox {
    let content_box = GtkBox::new(Orientation::Vertical, 0);

    let toolbar_container = create_toolbar();

    content_box.append(&toolbar_container);

    let separator = Separator::new(Orientation::Horizontal);
    content_box.append(&separator);

    let paned = Paned::new(Orientation::Vertical);

    let (list_view, store, statusbar) = create_package_list();
    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .child(&list_view)
        .build();

    paned.set_start_child(Some(&scrolled));

    let (info_panel, info_text) = create_info_panel();
    paned.set_end_child(Some(&info_panel));

    if let Some(selection_model) = list_view.model().and_downcast::<SingleSelection>() {
        selection_model.connect_selection_changed(clone!(
            #[weak]
            info_text,
            move |model, _position, _n_items| {
                if let Some(package_obj) =
                    model.selected_item().and_downcast::<PackageUpdateObject>()
                {
                    let package_data = package_obj.data();
                    info_text.set_text(package_data.description.as_str());
                } else {
                    info_text.set_text("Select a package to view its information.");
                }
            }
        ));
    }
    paned.set_position(410);

    content_box.append(&paned);

    update_statusbar(&statusbar, &store);
    content_box.append(&statusbar);

    return content_box;
}

pub fn load_packages(stack: Stack, content_box: GtkBox, window: ApplicationWindow) {
    glib::spawn_future_local(async move {
        let packages_result = gio::spawn_blocking(|| get_package_updates()).await;

        match packages_result {
            Ok(Ok(packages)) => {
                if packages.is_empty() {
                    stack.set_visible_child_name("no-updates");
                    return;
                }

                let paned = content_box
                    .last_child()
                    .and_then(|child| child.prev_sibling())
                    .and_downcast::<Paned>();

                let Some(paned) = paned else {
                    eprintln!("Could not find paned widget");
                    return;
                };

                let scrolled = paned.start_child().and_downcast::<ScrolledWindow>();
                let Some(scrolled) = scrolled else {
                    eprintln!("Could not find scrolled window");
                    return;
                };

                let column_view = scrolled.child().and_downcast::<ColumnView>();
                let Some(column_view) = column_view else {
                    eprintln!("Could not find column view");
                    return;
                };

                let selection_model = column_view.model();
                let Some(selection_model) = selection_model else {
                    eprintln!("Could not find selection model");
                    return;
                };

                let list_store = selection_model
                    .downcast_ref::<SingleSelection>()
                    .and_then(|sm| sm.model())
                    .and_downcast::<ListStore>();

                let Some(list_store) = list_store else {
                    eprintln!("Could not find list store");
                    return;
                };

                list_store.remove_all();

                for package in packages {
                    list_store.append(&PackageUpdateObject::new(package));
                }

                if let Some(statusbar) = content_box.last_child().and_downcast::<Statusbar>() {
                    update_statusbar(&statusbar, &list_store);
                }

                stack.set_visible_child_name("content");
            }
            Ok(Err(e)) => {
                show_error_dialog(
                    window.upcast_ref::<gtk4::Window>(),
                    "Error Loading Packages",
                    &format!("Failed to load package updates: {}", e),
                );
                eprintln!("Error loading packages: {}", e);
                stack.set_visible_child_name("content");
            }
            Err(e) => {
                eprintln!("Error in background thread: {:?}", e);
                stack.set_visible_child_name("content");
            }
        }
    });
}
