use gtk4::{ApplicationWindow, prelude::*};

use crate::helpers::elevated::open_url_as_user;
use crate::models::news_item::NewsItem;
use crate::ui::dialogs::build_dialog_window;

pub fn show_news_dialog(parent: &ApplicationWindow, items: &[NewsItem]) {
    let (dialog, content_area) = build_dialog_window(parent, "Arch Linux News", 560, 340);

    let list_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    list_box.set_margin_start(16);
    list_box.set_margin_end(16);
    list_box.set_margin_top(16);
    list_box.set_margin_bottom(16);

    for item in items {
        list_box.append(&build_news_entry(item));
    }

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .child(&list_box)
        .build();
    content_area.append(&scrolled);

    dialog.present();
}

fn build_news_entry(item: &NewsItem) -> gtk4::Box {
    let entry = gtk4::Box::new(gtk4::Orientation::Vertical, 4);

    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    title.set_wrap(true);
    let title_escaped = glib::markup_escape_text(&item.title);
    if item.link.is_empty() {
        title.set_markup(&format!("<b>{}</b>", title_escaped));
    } else {
        let link_escaped = glib::markup_escape_text(&item.link);
        title.set_markup(&format!(
            "<a href=\"{}\"><b>{}</b></a>",
            link_escaped, title_escaped
        ));
    }

    let link = item.link.clone();
    title.connect_activate_link(move |_, _| {
        if !link.is_empty() {
            open_url_as_user(&link);
        }
        return glib::Propagation::Stop;
    });
    entry.append(&title);

    let date = gtk4::Label::new(Some(&item.pub_date.format("%Y-%m-%d").to_string()));
    date.set_xalign(0.0);
    date.add_css_class("dim-label");
    date.add_css_class("caption");
    entry.append(&date);

    let snippet = first_sentence(&item.body);
    if !snippet.is_empty() {
        let body = gtk4::Label::new(Some(&snippet));
        body.set_xalign(0.0);
        body.set_wrap(true);
        body.set_margin_top(4);
        entry.append(&body);
    }

    let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    separator.set_margin_top(8);
    entry.append(&separator);

    return entry;
}

fn first_sentence(text: &str) -> String {
    let para = text
        .trim()
        .split("\n\n")
        .next()
        .unwrap_or("")
        .replace('\n', " ");
    let para = para.trim();
    if para.is_empty() {
        return String::new();
    }

    let indices: Vec<(usize, char)> = para.char_indices().collect();
    for (pos, (idx, c)) in indices.iter().enumerate() {
        if matches!(c, '.' | '!' | '?') {
            let next = indices.get(pos + 1).map(|(_, nc)| *nc);
            if next.map(|nc| nc.is_whitespace()).unwrap_or(true) {
                let end = idx + c.len_utf8();
                return para[..end].trim().to_string();
            }
        }
    }

    return para.to_string();
}
