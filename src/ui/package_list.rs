use crate::ui::package_object::PackageUpdateObject;
use gio::ListStore;
use glib::{clone, format_size};
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, CheckButton, ColumnView, ColumnViewColumn, Label, Orientation, SingleSelection,
    Statusbar,
};

pub fn create_package_list() -> (ColumnView, ListStore, Statusbar) {
    let store = ListStore::new::<PackageUpdateObject>();
    let statusbar = Statusbar::new();

    let selection_model = SingleSelection::new(Some(store.clone()));
    selection_model.set_autoselect(false);
    selection_model.set_can_unselect(true);

    let column_view = ColumnView::new(Some(selection_model));
    column_view.set_show_row_separators(true);
    column_view.set_show_column_separators(false);

    create_repository_column(&column_view);
    create_upgrade_column(&column_view, &store, &statusbar);
    create_name_column(&column_view);
    create_version_column(&column_view);
    create_size_column(&column_view);

    return (column_view, store, statusbar);
}

fn create_repository_column(column_view: &ColumnView) {
    let repository_factory = gtk4::SignalListItemFactory::new();
    repository_factory.connect_setup(move |_factory, item| {
        let label = Label::new(None);
        label.set_xalign(0.0);
        item.downcast_ref::<gtk4::ListItem>()
            .unwrap()
            .set_child(Some(&label));
    });
    repository_factory.connect_bind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let obj = list_item
            .item()
            .and_downcast::<PackageUpdateObject>()
            .unwrap();
        let data = obj.data();
        let label = list_item.child().and_downcast::<Label>().unwrap();

        label.set_text(&data.repository);

        if data.repository.contains("core") {
            label.set_markup(&format!("<b>{}</b>", &data.repository));
        } else if data.repository.contains("extra") {
            label.add_css_class("dim-label");
        }
    });
    let repository_column = ColumnViewColumn::new(Some("Repository"), Some(repository_factory));
    repository_column.set_resizable(true);
    column_view.append_column(&repository_column);
}

fn create_upgrade_column(column_view: &ColumnView, store: &ListStore, statusbar: &Statusbar) {
    let upgrade_factory = gtk4::SignalListItemFactory::new();
    upgrade_factory.connect_setup(move |_factory, item| {
        let check = CheckButton::new();
        check.set_halign(gtk4::Align::Center);
        item.downcast_ref::<gtk4::ListItem>()
            .unwrap()
            .set_child(Some(&check));
    });
    upgrade_factory.connect_bind(clone!(
        #[weak]
        store,
        #[weak]
        statusbar,
        move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let obj = list_item
                .item()
                .and_downcast::<PackageUpdateObject>()
                .unwrap();
            let data = obj.data();
            let check = list_item.child().and_downcast::<CheckButton>().unwrap();
            check.set_active(data.selected);

            check.connect_toggled(clone!(
                #[weak]
                obj,
                #[weak]
                store,
                #[weak]
                statusbar,
                move |check| {
                    obj.set_selected(check.is_active());
                    update_statusbar(&statusbar, &store);
                }
            ));
        }
    ));
    let upgrade_column = ColumnViewColumn::new(Some("Upgrade"), Some(upgrade_factory));
    column_view.append_column(&upgrade_column);
}

fn create_name_column(column_view: &ColumnView) {
    let name_factory = gtk4::SignalListItemFactory::new();
    name_factory.connect_setup(move |_factory, item| {
        let vbox = GtkBox::new(Orientation::Vertical, 2);
        let name_label = Label::new(None);
        name_label.set_xalign(0.0);
        name_label.set_css_classes(&["package-name"]);
        let desc_label = Label::new(None);
        desc_label.set_xalign(0.0);
        desc_label.set_css_classes(&["package-desc"]);
        desc_label.add_css_class("dim-label");
        vbox.append(&name_label);
        vbox.append(&desc_label);
        item.downcast_ref::<gtk4::ListItem>()
            .unwrap()
            .set_child(Some(&vbox));
    });
    name_factory.connect_bind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let obj = list_item
            .item()
            .and_downcast::<PackageUpdateObject>()
            .unwrap();
        let data = obj.data();
        let vbox = list_item.child().and_downcast::<GtkBox>().unwrap();
        let name_label = vbox.first_child().and_downcast::<Label>().unwrap();
        let desc_label = name_label.next_sibling().and_downcast::<Label>().unwrap();
        desc_label.set_wrap(true);

        name_label.set_text(&data.name);
        desc_label.set_text(&data.description);
    });
    let name_column = ColumnViewColumn::new(Some("Name"), Some(name_factory));
    name_column.set_expand(true);
    column_view.append_column(&name_column);
}

fn create_version_column(column_view: &ColumnView) {
    let version_factory = gtk4::SignalListItemFactory::new();
    version_factory.connect_setup(move |_factory, item| {
        let vbox = GtkBox::new(Orientation::Vertical, 2);
        let old_label = Label::new(None);
        old_label.set_xalign(0.0);
        old_label.add_css_class("dim-label");
        let new_label = Label::new(None);
        new_label.set_xalign(0.0);
        vbox.append(&old_label);
        vbox.append(&new_label);
        item.downcast_ref::<gtk4::ListItem>()
            .unwrap()
            .set_child(Some(&vbox));
    });
    version_factory.connect_bind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let obj = list_item
            .item()
            .and_downcast::<PackageUpdateObject>()
            .unwrap();
        let data = obj.data();
        let vbox = list_item.child().and_downcast::<GtkBox>().unwrap();
        let old_label = vbox.first_child().and_downcast::<Label>().unwrap();
        let new_label = old_label.next_sibling().and_downcast::<Label>().unwrap();

        old_label.set_text(&data.current_version);
        new_label.set_text(&data.new_version);
    });
    let version_column = ColumnViewColumn::new(Some("Version"), Some(version_factory));
    column_view.append_column(&version_column);
}

fn create_size_column(column_view: &ColumnView) {
    let size_factory = gtk4::SignalListItemFactory::new();
    size_factory.connect_setup(move |_factory, item| {
        let label = Label::new(None);
        label.set_xalign(0.0);
        item.downcast_ref::<gtk4::ListItem>()
            .unwrap()
            .set_child(Some(&label));
    });
    size_factory.connect_bind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let obj = list_item
            .item()
            .and_downcast::<PackageUpdateObject>()
            .unwrap();
        let data = obj.data();
        let label = list_item.child().and_downcast::<Label>().unwrap();

        let size_text = if data.size < 0 {
            format!("-{}", format_size(data.size.abs() as u64))
        } else {
            format_size(data.size as u64).to_string()
        };
        label.set_text(&size_text);
    });
    let size_column = ColumnViewColumn::new(Some("Update Size"), Some(size_factory));
    size_column.set_fixed_width(100);
    column_view.append_column(&size_column);
}

pub fn update_statusbar(statusbar: &Statusbar, store: &ListStore) {
    let context_id = statusbar.context_id("updates");

    statusbar.remove_all(context_id);

    let n_items = store.n_items();
    let mut selected_count = 0;
    let mut total_size = 0i64;

    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            let data = item.data();
            if data.selected {
                selected_count += 1;
                total_size += data.size;
            }
        }
    }

    let status_text = if total_size > 0 {
        let size_text = if total_size < 0 {
            format!("-{}", format_size(total_size.abs() as u64))
        } else {
            format_size(total_size as u64).to_string()
        };
        format!("{} updates selected ({})", selected_count, size_text)
    } else {
        format!("{} updates selected", selected_count)
    };

    statusbar.push(context_id, &status_text);
}
