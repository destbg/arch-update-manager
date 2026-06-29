use gio::ListStore;
use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, ColumnView, ColumnViewColumn, Label, ListItem,
    NoSelection, Orientation, PolicyType, ScrolledWindow, Spinner, TreeExpander, TreeListModel,
    TreeListRow,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::helpers::history::get_update_history;
use crate::log_info;
use crate::models::history_action::HistoryAction;
use crate::models::history_row::HistoryRow;
use crate::models::history_row_object::HistoryRowObject;
use crate::models::history_transaction::HistoryTransaction;
use crate::ui::dialogs::build_dialog_window;
use crate::ui::downgrade_dialog::show_downgrade_dialog;
use crate::ui::package_list::prefers_dark;

const HISTORY_LIMIT: usize = 50;

pub fn show_history_dialog(parent: &ApplicationWindow) {
    log_info!("history dialog opened");

    let (dialog, content_area) = build_dialog_window(parent, "Update history", 760, 560);
    content_area.append(&build_loading_view());
    dialog.present();

    let parent = parent.clone();
    let content_for_async = content_area.clone();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(|| get_update_history(HISTORY_LIMIT)).await;

        while let Some(child) = content_for_async.first_child() {
            content_for_async.remove(&child);
        }

        let body = match result {
            Ok(transactions) if transactions.is_empty() => {
                build_message_view("No update history was found in the pacman log.")
            }
            Ok(transactions) => build_table_view(&transactions, &parent),
            Err(_) => build_message_view("Could not read the pacman log."),
        };
        content_for_async.append(&body);
    });
}

fn build_table_view(transactions: &[HistoryTransaction], window: &ApplicationWindow) -> GtkBox {
    let dark = prefers_dark();

    let root = ListStore::new::<HistoryRowObject>();
    for transaction in transactions {
        root.append(&HistoryRowObject::new(HistoryRow::Transaction(
            transaction.clone(),
        )));
    }

    let tree_model = TreeListModel::new(root, false, false, |item| {
        let obj = item.downcast_ref::<HistoryRowObject>()?;
        match obj.row() {
            HistoryRow::Transaction(transaction) => {
                let children = ListStore::new::<HistoryRowObject>();
                for action in transaction.actions {
                    children.append(&HistoryRowObject::new(HistoryRow::Action(action)));
                }
                Some(children.upcast())
            }
            HistoryRow::Action(_) => None,
        }
    });

    let selection = NoSelection::new(Some(tree_model));
    let column_view = ColumnView::new(Some(selection));
    column_view.set_vexpand(true);
    column_view.set_hexpand(true);

    column_view.append_column(&build_when_column());
    column_view.append_column(&build_change_column(dark));
    column_view.append_column(&build_downgrade_column(window));

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&column_view)
        .build();

    let wrapper = GtkBox::new(Orientation::Vertical, 0);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);
    wrapper.append(&scrolled);
    return wrapper;
}

fn build_when_column() -> ColumnViewColumn {
    let factory = gtk4::SignalListItemFactory::new();

    factory.connect_setup(|_, item| {
        let Some(list_item) = item.downcast_ref::<ListItem>() else {
            return;
        };
        let expander = TreeExpander::new();
        let label = Label::new(None);
        label.set_xalign(0.0);
        expander.set_child(Some(&label));
        list_item.set_child(Some(&expander));
    });

    factory.connect_bind(|_, item| {
        let Some(list_item) = item.downcast_ref::<ListItem>() else {
            return;
        };
        let Some(tree_row) = list_item.item().and_downcast::<TreeListRow>() else {
            return;
        };
        let Some(expander) = list_item.child().and_downcast::<TreeExpander>() else {
            return;
        };
        expander.set_list_row(Some(&tree_row));

        let Some(label) = expander.child().and_downcast::<Label>() else {
            return;
        };
        let Some(obj) = tree_row.item().and_downcast::<HistoryRowObject>() else {
            return;
        };

        match obj.row() {
            HistoryRow::Transaction(transaction) => {
                label.set_markup(&format!(
                    "<b>{}</b>",
                    glib::markup_escape_text(&format_timestamp(&transaction.timestamp))
                ));
                label.set_tooltip_text(transaction.command.as_deref());
            }
            HistoryRow::Action(action) => {
                label.set_text(&action.package);
                label.set_tooltip_text(None);
            }
        }
    });

    let column = ColumnViewColumn::new(Some("When"), Some(factory));
    column.set_resizable(true);
    column.set_fixed_width(220);
    return column;
}

fn build_change_column(dark: bool) -> ColumnViewColumn {
    let factory = gtk4::SignalListItemFactory::new();

    factory.connect_setup(|_, item| {
        let Some(list_item) = item.downcast_ref::<ListItem>() else {
            return;
        };
        let label = Label::new(None);
        label.set_xalign(0.0);
        label.set_wrap(true);
        list_item.set_child(Some(&label));
    });

    factory.connect_bind(move |_, item| {
        let Some(list_item) = item.downcast_ref::<ListItem>() else {
            return;
        };
        let Some(tree_row) = list_item.item().and_downcast::<TreeListRow>() else {
            return;
        };
        let Some(label) = list_item.child().and_downcast::<Label>() else {
            return;
        };
        let Some(obj) = tree_row.item().and_downcast::<HistoryRowObject>() else {
            return;
        };

        match obj.row() {
            HistoryRow::Transaction(transaction) => {
                label.set_markup(&format!(
                    "<span foreground=\"{}\">{}</span>",
                    dim_color(dark),
                    glib::markup_escape_text(&transaction.summary())
                ));
            }
            HistoryRow::Action(action) => {
                label.set_markup(&action_change_markup(&action, dark));
            }
        }
    });

    let column = ColumnViewColumn::new(Some("Change"), Some(factory));
    column.set_expand(true);
    return column;
}

fn build_downgrade_column(window: &ApplicationWindow) -> ColumnViewColumn {
    let factory = gtk4::SignalListItemFactory::new();
    let window = window.clone();

    factory.connect_setup(move |_, item| {
        let Some(list_item) = item.downcast_ref::<ListItem>() else {
            return;
        };

        let button = Button::with_label("Downgrade");
        button.add_css_class("flat");
        button.set_halign(Align::End);
        button.set_valign(Align::Center);
        button.set_visible(false);

        let state: Rc<RefCell<Option<(String, String)>>> = Rc::new(RefCell::new(None));
        let window = window.clone();
        let state_for_click = state.clone();
        button.connect_clicked(move |_| {
            if let Some((package, version)) = state_for_click.borrow().clone() {
                log_info!("history: downgrade {}", package);
                show_downgrade_dialog(&window, &package, &version);
            }
        });

        unsafe {
            button.set_data("dg_state", state);
        }
        list_item.set_child(Some(&button));
    });

    factory.connect_bind(|_, item| {
        let Some(list_item) = item.downcast_ref::<ListItem>() else {
            return;
        };
        let Some(tree_row) = list_item.item().and_downcast::<TreeListRow>() else {
            return;
        };
        let Some(button) = list_item.child().and_downcast::<Button>() else {
            return;
        };
        let Some(obj) = tree_row.item().and_downcast::<HistoryRowObject>() else {
            return;
        };

        match obj.row() {
            HistoryRow::Action(action)
                if can_downgrade(&action) && action.new_version.is_some() =>
            {
                let version = action.new_version.clone().unwrap_or_default();
                unsafe {
                    if let Some(state) =
                        button.data::<Rc<RefCell<Option<(String, String)>>>>("dg_state")
                    {
                        *state.as_ref().borrow_mut() = Some((action.package.clone(), version));
                    }
                }
                button.set_visible(true);
            }
            _ => {
                button.set_visible(false);
            }
        }
    });

    let column = ColumnViewColumn::new(Some(""), Some(factory));
    column.set_fixed_width(130);
    return column;
}

fn can_downgrade(action: &HistoryAction) -> bool {
    return matches!(
        action.action.as_str(),
        "upgraded" | "downgraded" | "reinstalled"
    );
}

fn action_change_markup(action: &HistoryAction, dark: bool) -> String {
    let color = action_color(&action.action, dark);
    let verb = glib::markup_escape_text(&action.action);

    let versions = match (&action.old_version, &action.new_version) {
        (Some(old), Some(new)) => format!(
            "  <span foreground=\"{}\">{} -> {}</span>",
            dim_color(dark),
            glib::markup_escape_text(old),
            glib::markup_escape_text(new)
        ),
        (Some(only), None) | (None, Some(only)) => format!(
            "  <span foreground=\"{}\">{}</span>",
            dim_color(dark),
            glib::markup_escape_text(only)
        ),
        (None, None) => String::new(),
    };

    return format!("<span foreground=\"{}\">{}</span>{}", color, verb, versions);
}

fn action_color(action: &str, dark: bool) -> &'static str {
    return match action {
        "upgraded" => {
            if dark {
                "#62a0ea"
            } else {
                "#3584e4"
            }
        }
        "installed" => {
            if dark {
                "#5adc82"
            } else {
                "#2a9d4a"
            }
        }
        "removed" => {
            if dark {
                "#f66151"
            } else {
                "#e01b24"
            }
        }
        "downgraded" => {
            if dark {
                "#ffa348"
            } else {
                "#e66100"
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

fn dim_color(dark: bool) -> &'static str {
    return if dark { "#a0a0a0" } else { "#6a6a6a" };
}

fn format_timestamp(ts: &str) -> String {
    let cleaned = ts.replace('T', " ");
    if cleaned.len() >= 16 {
        return cleaned[..16].to_string();
    }
    return cleaned;
}

fn build_loading_view() -> GtkBox {
    let wrapper = GtkBox::new(Orientation::Vertical, 12);
    wrapper.set_valign(Align::Center);
    wrapper.set_halign(Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let spinner = Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.start();
    wrapper.append(&spinner);

    let label = Label::new(Some("Reading the pacman log..."));
    label.add_css_class("dim-label");
    wrapper.append(&label);

    return wrapper;
}

fn build_message_view(message: &str) -> GtkBox {
    let wrapper = GtkBox::new(Orientation::Vertical, 12);
    wrapper.set_valign(Align::Center);
    wrapper.set_halign(Align::Center);
    wrapper.set_vexpand(true);
    wrapper.set_hexpand(true);

    let label = Label::new(Some(message));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    label.add_css_class("dim-label");
    wrapper.append(&label);

    return wrapper;
}
