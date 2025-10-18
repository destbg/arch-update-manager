use crate::helpers::package_updates::get_package_updates;
use crate::ui::dialogs::show_error_dialog;
use crate::ui::info_panel::create_info_panel;
use crate::ui::loading::create_loading_page;
use crate::ui::package_list::{create_package_list, update_statusbar};
use crate::ui::package_object::PackageUpdateObject;
use crate::ui::toolbar::create_toolbar;
use gio::ListStore;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, ColumnView, Orientation, Paned, ScrolledWindow,
    Separator, SingleSelection, Stack, Statusbar,
};

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Arch Update Manager")
        .icon_name("arch-update-manager")
        .default_width(900)
        .default_height(600)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 0);

    let stack = Stack::new();
    stack.set_vexpand(true);

    let loading_box = create_loading_page();
    stack.add_named(&loading_box, Some("loading"));

    let content_box = create_main_content();
    stack.add_named(&content_box, Some("content"));

    main_box.append(&stack);

    window.set_child(Some(&main_box));

    stack.set_visible_child_name("loading");

    window.present();

    glib::idle_add_local_once(clone!(
        #[weak]
        stack,
        #[weak]
        content_box,
        #[weak]
        window,
        move || {
            load_packages(stack, content_box, window);
        }
    ));
}

fn create_main_content() -> GtkBox {
    let content_box = GtkBox::new(Orientation::Vertical, 0);

    let (toolbar_container, _timeshift_checkbox) = create_toolbar();
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
    paned.set_position(380);

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
                if let Some(paned) = content_box
                    .last_child()
                    .and_then(|child| child.prev_sibling())
                    .and_downcast::<Paned>()
                {
                    if let Some(scrolled) = paned.start_child().and_downcast::<ScrolledWindow>() {
                        if let Some(column_view) = scrolled.child().and_downcast::<ColumnView>() {
                            if let Some(selection_model) = column_view.model() {
                                if let Some(list_store) = selection_model
                                    .downcast_ref::<SingleSelection>()
                                    .and_then(|sm| sm.model())
                                    .and_downcast::<ListStore>()
                                {
                                    list_store.remove_all();

                                    for package in packages {
                                        list_store.append(&PackageUpdateObject::new(package));
                                    }

                                    if let Some(statusbar) =
                                        content_box.last_child().and_downcast::<Statusbar>()
                                    {
                                        update_statusbar(&statusbar, &list_store);
                                    }
                                }
                            }
                        }
                    }
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
