use crate::constants::is_own_package;
use crate::helpers::settings::{load_settings, save_settings};
use crate::helpers::tray_integration::kick_tray;
use crate::helpers::unselected_packages::save_unselected_packages;
use crate::log_info;
use crate::models::package_object::PackageUpdateObject;
use crate::models::package_source::PackageSource;
use crate::models::package_update::PackageUpdate;
use crate::ui::context_menu::show_package_context_menu;
use gio::ListStore;
use glib::{WeakRef, clone, format_size};
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, CheckButton, ColumnView, ColumnViewColumn, CustomFilter, CustomSorter,
    EventSequenceState, FilterListModel, GestureClick, Label, ListItem, Ordering, Orientation,
    PropagationPhase, SearchEntry, SingleSelection, SortListModel, ToggleButton, gdk,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    static FAVORITE_BUTTONS: RefCell<HashMap<String, WeakRef<ToggleButton>>> =
        RefCell::new(HashMap::new());
}

pub fn refresh_favorite_button(name: &str, is_favorite: bool) {
    FAVORITE_BUTTONS.with(|map| {
        let map = map.borrow();
        let Some(weak) = map.get(name) else {
            return;
        };
        apply_favorite_state(weak, is_favorite);
    });
}

pub fn refresh_all_favorite_buttons(is_favorite: bool) {
    FAVORITE_BUTTONS.with(|map| {
        for weak in map.borrow().values() {
            apply_favorite_state(weak, is_favorite);
        }
    });
}

pub fn create_package_list(
    search_entry: &SearchEntry,
) -> (ColumnView, ListStore, Label, CustomFilter) {
    let store = ListStore::new::<PackageUpdateObject>();
    let statusbar = Label::new(None);
    statusbar.set_xalign(0.0);
    statusbar.set_margin_start(10);
    statusbar.set_margin_end(10);
    statusbar.set_margin_top(4);
    statusbar.set_margin_bottom(4);
    statusbar.add_css_class("dim-label");

    let column_view = ColumnView::new(None::<SingleSelection>);
    column_view.set_show_row_separators(true);
    column_view.set_show_column_separators(false);

    let entry_for_filter = search_entry.clone();
    let filter = CustomFilter::new(move |obj| {
        let query = entry_for_filter.text().to_string();
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }
        let Some(pkg) = obj.downcast_ref::<PackageUpdateObject>() else {
            return true;
        };
        let data = pkg.data();
        let needle = trimmed.to_lowercase();
        return data.name.to_lowercase().contains(&needle)
            || data.description.to_lowercase().contains(&needle);
    });

    let sort_model = SortListModel::new(Some(store.clone()), column_view.sorter());
    let filter_model = FilterListModel::new(Some(sort_model), Some(filter.clone()));
    let selection_model = SingleSelection::new(Some(filter_model));
    selection_model.set_autoselect(false);
    selection_model.set_can_unselect(true);
    column_view.set_model(Some(&selection_model));

    create_favorite_column(&column_view);
    create_repository_column(&column_view);
    create_upgrade_column(&column_view, &store, &statusbar);
    create_name_column(&column_view);
    create_version_column(&column_view);
    create_size_column(&column_view);

    return (column_view, store, statusbar, filter);
}

pub fn update_statusbar(statusbar: &Label, store: &ListStore) {
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

    statusbar.set_text(&status_text);
}

pub fn save_unselected_from_store(store: &ListStore) {
    let settings = load_settings();
    if !settings.remember_unselected_packages {
        return;
    }

    let n_items = store.n_items();
    let mut unselected = Vec::new();

    for i in 0..n_items {
        if let Some(item) = store.item(i).and_downcast::<PackageUpdateObject>() {
            let data = item.data();
            if !data.selected {
                unselected.push(data.name.clone());
            }
        }
    }

    save_unselected_packages(unselected);
}

pub(crate) fn prefers_dark() -> bool {
    return gtk4::Settings::default()
        .map(|s| s.is_gtk_application_prefer_dark_theme())
        .unwrap_or(false);
}

pub(crate) fn severity_color(severity: &str, dark: bool) -> &'static str {
    return match severity.to_ascii_lowercase().as_str() {
        "critical" => {
            if dark {
                "#f66151"
            } else {
                "#e01b24"
            }
        }
        "high" => {
            if dark {
                "#ffa348"
            } else {
                "#e66100"
            }
        }
        "medium" => {
            if dark {
                "#f5c211"
            } else {
                "#e5a50a"
            }
        }
        "low" => {
            if dark {
                "#62a0ea"
            } else {
                "#3584e4"
            }
        }
        _ => {
            if dark {
                "#c0bfbc"
            } else {
                "#9a9996"
            }
        }
    };
}

pub(crate) fn format_build_date(timestamp: i64) -> String {
    use chrono::{Local, TimeZone, Utc};

    const HOUR: i64 = 3600;
    const DAY: i64 = 24 * HOUR;
    const WEEK: i64 = 7 * DAY;

    let absolute_date = || {
        Local
            .timestamp_opt(timestamp, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default()
    };

    let diff = Utc::now().timestamp() - timestamp;
    if diff < 0 || diff >= WEEK {
        return absolute_date();
    }

    if diff < HOUR {
        let minutes = diff / 60;
        if minutes < 1 {
            return "just now".to_string();
        }
        return format!("{} minute{} ago", minutes, plural(minutes));
    }

    if diff < DAY {
        let hours = diff / HOUR;
        return format!("{} hour{} ago", hours, plural(hours));
    }

    let days = diff / DAY;
    return format!("{} day{} ago", days, plural(days));
}

pub(crate) fn is_recently_created(first_submitted: Option<i64>) -> bool {
    const WEEK: i64 = 7 * 24 * 3600;
    let Some(ts) = first_submitted else {
        return false;
    };
    let diff = chrono::Utc::now().timestamp() - ts;
    return diff >= 0 && diff < WEEK;
}

pub(crate) fn format_age(timestamp: i64) -> String {
    const DAY: i64 = 24 * 3600;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;

    let diff = chrono::Utc::now().timestamp() - timestamp;
    if diff < 0 {
        return "just now".to_string();
    }
    if diff >= YEAR {
        let years = diff / YEAR;
        return format!("{} year{} ago", years, plural(years));
    }
    if diff >= MONTH {
        let months = diff / MONTH;
        return format!("{} month{} ago", months, plural(months));
    }
    let days = diff / DAY;
    if days < 1 {
        return "today".to_string();
    }
    return format!("{} day{} ago", days, plural(days));
}

fn apply_favorite_state(weak: &WeakRef<ToggleButton>, is_favorite: bool) {
    let Some(button) = weak.upgrade() else {
        return;
    };
    let handler = unsafe { button.steal_data::<glib::SignalHandlerId>("fav_handler") };
    if let Some(handler_id) = handler {
        button.block_signal(&handler_id);
        button.set_active(is_favorite);
        button.set_icon_name(if is_favorite {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        });
        button.unblock_signal(&handler_id);
        unsafe {
            button.set_data("fav_handler", handler_id);
        }
    } else {
        button.set_active(is_favorite);
        button.set_icon_name(if is_favorite {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        });
    }
}

fn package_sorter<F>(key: F) -> CustomSorter
where
    F: Fn(&PackageUpdateObject, &PackageUpdateObject) -> std::cmp::Ordering + 'static,
{
    return CustomSorter::new(move |a, b| {
        let a = a.downcast_ref::<PackageUpdateObject>();
        let b = b.downcast_ref::<PackageUpdateObject>();
        return match (a, b) {
            (Some(a), Some(b)) => key(a, b).into(),
            _ => Ordering::Equal,
        };
    });
}

fn attach_deselect_gesture(
    cell: &impl IsA<gtk4::Widget>,
    column_view: &ColumnView,
    list_item: &ListItem,
) {
    let gesture = GestureClick::new();
    gesture.set_button(gdk::BUTTON_PRIMARY);
    gesture.set_propagation_phase(PropagationPhase::Capture);
    let column_view = column_view.clone();
    let list_item = list_item.clone();
    gesture.connect_pressed(move |gesture, _n_press, _x, _y| {
        let position = list_item.position();
        if position == gtk4::INVALID_LIST_POSITION {
            return;
        }
        let Some(model) = column_view.model().and_downcast::<SingleSelection>() else {
            return;
        };
        if model.selected() == position {
            model.set_selected(gtk4::INVALID_LIST_POSITION);
            gesture.set_state(EventSequenceState::Claimed);
        }
    });
    cell.add_controller(gesture);
}

fn create_favorite_column(column_view: &ColumnView) {
    let css = gtk4::CssProvider::new();
    css.load_from_data(
        "button.favorite-star,
         button.favorite-star:checked {
             background-color: transparent;
             box-shadow: none;
         }
         button.favorite-star:hover,
         button.favorite-star:checked:hover {
             background-color: alpha(currentColor, 0.08);
             box-shadow: none;
         }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let factory = gtk4::SignalListItemFactory::new();
    let shift_held: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let last_anchor: Rc<RefCell<Option<u32>>> = Rc::new(RefCell::new(None));

    factory.connect_setup(clone!(
        #[strong]
        shift_held,
        move |_factory, item| {
            let button = ToggleButton::new();
            button.set_halign(gtk4::Align::Center);
            button.set_icon_name("non-starred-symbolic");
            button.add_css_class("flat");
            button.add_css_class("favorite-star");

            let shift_for_capture = shift_held.clone();
            let gesture = GestureClick::new();
            gesture.set_propagation_phase(PropagationPhase::Capture);
            gesture.connect_pressed(move |g, _n_press, _x, _y| {
                let modifier = g.current_event_state();
                *shift_for_capture.borrow_mut() = modifier.contains(gdk::ModifierType::SHIFT_MASK);
            });
            button.add_controller(gesture);

            item.downcast_ref::<gtk4::ListItem>()
                .unwrap()
                .set_child(Some(&button));
        }
    ));

    factory.connect_bind(clone!(
        #[strong]
        shift_held,
        #[strong]
        last_anchor,
        #[strong]
        column_view,
        move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap().clone();
            let obj = list_item
                .item()
                .and_downcast::<PackageUpdateObject>()
                .unwrap();
            let pkg_name = obj.data().name.clone();
            let button = list_item.child().and_downcast::<ToggleButton>().unwrap();

            let is_fav = load_settings().is_favorite(&pkg_name);
            button.set_active(is_fav);
            button.set_icon_name(if is_fav {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            });

            let handler_id = button.connect_toggled(clone!(
                #[strong]
                pkg_name,
                #[strong]
                list_item,
                #[strong]
                shift_held,
                #[strong]
                last_anchor,
                #[strong]
                column_view,
                move |btn| {
                    let is_active = btn.is_active();
                    log_info!(
                        "favorite toggled: {} -> {}",
                        pkg_name,
                        if is_active {
                            "favorite"
                        } else {
                            "not favorite"
                        }
                    );
                    btn.set_icon_name(if is_active {
                        "starred-symbolic"
                    } else {
                        "non-starred-symbolic"
                    });
                    let mut s = load_settings();
                    s.set_favorite(&pkg_name, is_active);
                    if save_settings(&s).is_ok() {
                        kick_tray();
                    }

                    let current_pos = list_item.position();
                    let shift = *shift_held.borrow();
                    *shift_held.borrow_mut() = false;

                    if shift {
                        let anchor = *last_anchor.borrow();
                        if let Some(anchor_pos) = anchor {
                            if anchor_pos != current_pos {
                                apply_favorite_range(
                                    &column_view,
                                    anchor_pos,
                                    current_pos,
                                    is_active,
                                );
                            }
                        }
                    } else {
                        *last_anchor.borrow_mut() = Some(current_pos);
                    }
                }
            ));

            unsafe {
                button.set_data("fav_handler", handler_id);
                button.set_data("fav_pkg_name", pkg_name.clone());
            }

            FAVORITE_BUTTONS.with(|map| {
                map.borrow_mut().insert(pkg_name, button.downgrade());
            });
        }
    ));

    factory.connect_unbind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let button = list_item.child().and_downcast::<ToggleButton>().unwrap();
        unsafe {
            if let Some(handler_id) = button.steal_data::<glib::SignalHandlerId>("fav_handler") {
                button.disconnect(handler_id);
            }
            if let Some(name) = button.steal_data::<String>("fav_pkg_name") {
                FAVORITE_BUTTONS.with(|map| {
                    map.borrow_mut().remove(&name);
                });
            }
        }
    });

    let column = ColumnViewColumn::new(Some("Favorite"), Some(factory));
    column.set_fixed_width(62);
    let settings = load_settings();
    column.set_visible(settings.enable_favorites && settings.show_favorites_column);
    column_view.append_column(&column);
}

fn create_repository_column(column_view: &ColumnView) {
    let repository_factory = gtk4::SignalListItemFactory::new();
    let column_view_for_gesture = column_view.clone();
    repository_factory.connect_setup(move |_factory, item| {
        let label = Label::new(None);
        label.set_xalign(0.0);
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        attach_deselect_gesture(&label, &column_view_for_gesture, list_item);
        list_item.set_child(Some(&label));
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

        if data.source == PackageSource::Aur {
            label.set_markup(&format!("<b>{}</b>", &data.repository));
        } else if data.repository.contains("extra") {
            label.add_css_class("dim-label");
        }
    });
    let repository_column = ColumnViewColumn::new(Some("Repository"), Some(repository_factory));
    repository_column.set_resizable(true);
    repository_column.set_sorter(Some(&package_sorter(|a, b| {
        let a = a.data();
        let b = b.data();
        return a
            .repository
            .cmp(&b.repository)
            .then_with(|| a.name.cmp(&b.name));
    })));
    column_view.append_column(&repository_column);
}

fn create_upgrade_column(column_view: &ColumnView, store: &ListStore, statusbar: &Label) {
    let upgrade_factory = gtk4::SignalListItemFactory::new();
    let shift_held: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let last_anchor: Rc<RefCell<Option<u32>>> = Rc::new(RefCell::new(None));

    upgrade_factory.connect_setup(clone!(
        #[strong]
        store,
        #[strong]
        statusbar,
        #[strong]
        shift_held,
        #[strong]
        last_anchor,
        #[strong]
        column_view,
        move |_factory, item| {
            let list_item = item.downcast_ref::<ListItem>().unwrap().clone();
            let check = CheckButton::new();
            check.set_halign(gtk4::Align::Center);

            let shift_for_capture = shift_held.clone();
            let gesture = GestureClick::new();
            gesture.set_propagation_phase(PropagationPhase::Capture);
            gesture.connect_pressed(move |g, _n_press, _x, _y| {
                let modifier = g.current_event_state();
                *shift_for_capture.borrow_mut() = modifier.contains(gdk::ModifierType::SHIFT_MASK);
            });
            check.add_controller(gesture);

            let handler_id = check.connect_toggled(clone!(
                #[strong]
                list_item,
                #[strong]
                store,
                #[strong]
                statusbar,
                #[strong]
                shift_held,
                #[strong]
                last_anchor,
                #[strong]
                column_view,
                move |check| {
                    let Some(obj) = list_item.item().and_downcast::<PackageUpdateObject>() else {
                        return;
                    };
                    let active = check.is_active();
                    let name = obj.data().name;
                    log_info!(
                        "package toggled: {} -> {}",
                        name,
                        if active { "selected" } else { "unselected" }
                    );
                    obj.set_selected(active);

                    let current_pos = list_item.position();
                    let shift = *shift_held.borrow();
                    *shift_held.borrow_mut() = false;

                    if shift {
                        let anchor = *last_anchor.borrow();
                        if let Some(anchor_pos) = anchor {
                            if anchor_pos != current_pos {
                                apply_range_selection(
                                    &column_view,
                                    &store,
                                    anchor_pos,
                                    current_pos,
                                    active,
                                );
                            }
                        }
                    } else {
                        *last_anchor.borrow_mut() = Some(current_pos);
                    }

                    update_statusbar(&statusbar, &store);
                    save_unselected_from_store(&store);
                }
            ));

            unsafe {
                check.set_data("upgrade_handler", handler_id);
            }

            list_item.set_child(Some(&check));
        }
    ));

    upgrade_factory.connect_bind(move |_factory, item| {
        let list_item = item.downcast_ref::<ListItem>().unwrap();
        let Some(obj) = list_item.item().and_downcast::<PackageUpdateObject>() else {
            return;
        };
        let Some(check) = list_item.child().and_downcast::<CheckButton>() else {
            return;
        };
        let handler = unsafe { check.steal_data::<glib::SignalHandlerId>("upgrade_handler") };
        let selected = obj.data().selected;
        if let Some(hid) = handler {
            check.block_signal(&hid);
            check.set_active(selected);
            check.unblock_signal(&hid);
            unsafe {
                check.set_data("upgrade_handler", hid);
            }
        } else {
            check.set_active(selected);
        }
    });

    let upgrade_column = ColumnViewColumn::new(Some("Upgrade"), Some(upgrade_factory));
    column_view.append_column(&upgrade_column);
}

fn apply_range_selection(
    column_view: &ColumnView,
    store: &ListStore,
    anchor_pos: u32,
    current_pos: u32,
    new_state: bool,
) {
    let Some(model) = column_view.model() else {
        return;
    };
    let (lo, hi) = if anchor_pos < current_pos {
        (anchor_pos, current_pos)
    } else {
        (current_pos, anchor_pos)
    };
    for p in lo..=hi {
        if let Some(item) = model.item(p).and_downcast::<PackageUpdateObject>() {
            item.set_selected(new_state);
        }
    }

    let items: Vec<PackageUpdateObject> = (0..store.n_items())
        .filter_map(|i| store.item(i).and_downcast::<PackageUpdateObject>())
        .collect();
    store.remove_all();
    for item in items {
        store.append(&item);
    }
}

fn apply_favorite_range(
    column_view: &ColumnView,
    anchor_pos: u32,
    current_pos: u32,
    new_state: bool,
) {
    let Some(model) = column_view.model() else {
        return;
    };
    let (lo, hi) = if anchor_pos < current_pos {
        (anchor_pos, current_pos)
    } else {
        (current_pos, anchor_pos)
    };

    let mut settings = load_settings();
    let mut changed = Vec::new();
    for p in lo..=hi {
        if let Some(item) = model.item(p).and_downcast::<PackageUpdateObject>() {
            let name = item.data().name;
            if settings.is_favorite(&name) != new_state {
                settings.set_favorite(&name, new_state);
                changed.push(name);
            }
        }
    }

    if changed.is_empty() {
        return;
    }

    if save_settings(&settings).is_ok() {
        for name in &changed {
            refresh_favorite_button(name, new_state);
        }
        kick_tray();
    }
}

fn create_name_column(column_view: &ColumnView) {
    let name_factory = gtk4::SignalListItemFactory::new();
    let column_view_for_gesture = column_view.clone();
    name_factory.connect_setup(move |_factory, item| {
        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_valign(gtk4::Align::Center);

        let name_row = GtkBox::new(Orientation::Horizontal, 6);
        let name_label = Label::new(None);
        name_label.set_xalign(0.0);
        name_label.set_hexpand(true);
        name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        name_label.set_css_classes(&["package-name"]);
        let updated_label = Label::new(None);
        updated_label.set_halign(gtk4::Align::End);
        updated_label.set_valign(gtk4::Align::Center);
        updated_label.add_css_class("dim-label");
        updated_label.add_css_class("caption");
        name_row.append(&name_label);
        name_row.append(&updated_label);

        let desc_label = Label::new(None);
        desc_label.set_xalign(0.0);
        desc_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        desc_label.set_max_width_chars(50);
        desc_label.set_css_classes(&["package-desc"]);
        desc_label.add_css_class("dim-label");

        vbox.append(&name_row);
        vbox.append(&desc_label);

        let row_package: Rc<RefCell<Option<PackageUpdate>>> = Rc::new(RefCell::new(None));
        let gesture = GestureClick::new();
        gesture.set_button(gdk::BUTTON_SECONDARY);
        let row_package_for_gesture = row_package.clone();
        let vbox_for_gesture = vbox.clone();
        gesture.connect_pressed(move |_, _n_press, x, y| {
            let Some(pkg) = row_package_for_gesture.borrow().clone() else {
                return;
            };
            show_package_context_menu(vbox_for_gesture.upcast_ref::<gtk4::Widget>(), &pkg, x, y);
        });
        vbox.add_controller(gesture);
        unsafe {
            vbox.set_data("ctx_pkg", row_package);
        }

        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        attach_deselect_gesture(&vbox, &column_view_for_gesture, list_item);
        list_item.set_child(Some(&vbox));
    });
    name_factory.connect_bind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let obj = list_item
            .item()
            .and_downcast::<PackageUpdateObject>()
            .unwrap();
        let data = obj.data();
        let vbox = list_item.child().and_downcast::<GtkBox>().unwrap();
        let name_row = vbox.first_child().and_downcast::<GtkBox>().unwrap();
        let name_label = name_row.first_child().and_downcast::<Label>().unwrap();
        let updated_label = name_label.next_sibling().and_downcast::<Label>().unwrap();
        let desc_label = name_row.next_sibling().and_downcast::<Label>().unwrap();

        name_label.set_markup(&name_markup(&data));
        desc_label.set_text(&data.description);

        let mut tooltip_parts: Vec<String> = data.security_issues.clone();
        if !is_own_package(&data.name) {
            for finding in &data.aur_scan_findings {
                tooltip_parts.push(format!("aur-scan: {}", finding.title));
            }
        }
        if tooltip_parts.is_empty() {
            name_label.set_tooltip_text(None);
        } else {
            name_label.set_tooltip_text(Some(&tooltip_parts.join("\n")));
        }

        unsafe {
            if let Some(state) = vbox.data::<Rc<RefCell<Option<PackageUpdate>>>>("ctx_pkg") {
                *state.as_ref().borrow_mut() = Some(data.clone());
            }
        }

        let settings = load_settings();
        desc_label.set_visible(settings.show_package_descriptions);

        match data.build_date {
            Some(ts) if settings.show_updated_date => {
                updated_label.set_text(&format_build_date(ts));
                updated_label.set_visible(true);
            }
            _ => {
                updated_label.set_text("");
                updated_label.set_visible(false);
            }
        }
    });
    name_factory.connect_unbind(move |_factory, item| {
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let Some(vbox) = list_item.child().and_downcast::<GtkBox>() else {
            return;
        };
        unsafe {
            if let Some(state) = vbox.data::<Rc<RefCell<Option<PackageUpdate>>>>("ctx_pkg") {
                *state.as_ref().borrow_mut() = None;
            }
        }
    });
    let name_column = ColumnViewColumn::new(Some("Name"), Some(name_factory));
    name_column.set_expand(true);
    name_column.set_sorter(Some(&package_sorter(|a, b| {
        return a.data().name.cmp(&b.data().name);
    })));
    column_view.append_column(&name_column);
}

fn create_version_column(column_view: &ColumnView) {
    let version_factory = gtk4::SignalListItemFactory::new();
    let column_view_for_gesture = column_view.clone();
    version_factory.connect_setup(move |_factory, item| {
        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_valign(gtk4::Align::Center);
        let old_label = Label::new(None);
        old_label.set_xalign(0.0);
        old_label.add_css_class("dim-label");
        let new_label = Label::new(None);
        new_label.set_xalign(0.0);
        vbox.append(&old_label);
        vbox.append(&new_label);
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        attach_deselect_gesture(&vbox, &column_view_for_gesture, list_item);
        list_item.set_child(Some(&vbox));
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
        old_label.set_visible(
            !data.current_version.is_empty() && data.current_version != data.new_version,
        );
    });
    let version_column = ColumnViewColumn::new(Some("Version"), Some(version_factory));
    version_column.set_sorter(Some(&package_sorter(|a, b| {
        return a.data().new_version.cmp(&b.data().new_version);
    })));
    column_view.append_column(&version_column);
}

fn create_size_column(column_view: &ColumnView) {
    let size_factory = gtk4::SignalListItemFactory::new();
    let column_view_for_gesture = column_view.clone();
    size_factory.connect_setup(move |_factory, item| {
        let label = Label::new(None);
        label.set_xalign(0.0);
        let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        attach_deselect_gesture(&label, &column_view_for_gesture, list_item);
        list_item.set_child(Some(&label));
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
            format!("-{}", format_size(data.size.unsigned_abs()))
        } else {
            format_size(data.size as u64).to_string()
        };
        label.set_text(&size_text);
    });
    let size_column = ColumnViewColumn::new(Some("Update Size"), Some(size_factory));
    size_column.set_fixed_width(100);
    size_column.set_sorter(Some(&package_sorter(|a, b| {
        return a.data().size.cmp(&b.data().size);
    })));
    column_view.append_column(&size_column);
}

fn name_markup(data: &PackageUpdate) -> String {
    let dark = prefers_dark();
    let mut markup = glib::markup_escape_text(&data.name).to_string();

    if is_recently_created(data.first_submitted) {
        markup.push_str(&badge("new", if dark { "#ff6b6b" } else { "#e01b24" }));
    }
    if data.maintainer_changed() {
        markup.push_str(&badge(
            "maintainer changed",
            if dark { "#ffa348" } else { "#e66100" },
        ));
    }
    if !data.new_permissions.is_empty() {
        markup.push_str(&badge(
            "new permissions",
            if dark { "#ffa348" } else { "#e66100" },
        ));
    }
    if data.pkgbuild_needs_review {
        markup.push_str(&badge(
            "review PKGBUILD",
            if dark { "#ffa348" } else { "#e66100" },
        ));
    }
    if data.orphaned {
        markup.push_str(&badge("orphaned", if dark { "#f5c211" } else { "#e5a50a" }));
    }
    if data.out_of_date.is_some() {
        markup.push_str(&badge(
            "out of date",
            if dark { "#c0bfbc" } else { "#9a9996" },
        ));
    }
    if let Some(severity) = &data.security_severity {
        markup.push_str(&badge(severity, severity_color(severity, dark)));
    }
    if !is_own_package(&data.name) {
        if let Some((severity, count)) = data.aur_scan_summary() {
            markup.push_str(&badge(
                &format!("aur-scan: {} ({})", severity, count),
                severity_color(&severity, dark),
            ));
        }
    }

    return markup;
}

fn badge(text: &str, color: &str) -> String {
    let safe = glib::markup_escape_text(text);
    return format!(" <span foreground=\"{}\">[{}]</span>", color, safe);
}

fn plural(n: i64) -> &'static str {
    return if n == 1 { "" } else { "s" };
}
